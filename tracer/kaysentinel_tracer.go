// Package kaysentinel implements the EMES-V1 client telemetry tracer against
// go-ethereum's current live-tracing API (core/tracing.Hooks), NOT the
// deprecated vm.EVMLogger interface that was removed from go-ethereum in the
// "live tracing" overhaul. See docs/emes_profile.md for the discrepancy this
// replaces and why.
package kaysentinel

import (
	"encoding/json"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/tracing"
	"github.com/ethereum/go-ethereum/core/types"
)

// --- EMES-V1-RC3 wire event types (unchanged from the frozen spec) ---

type FrameType uint8

const (
	FrameCall FrameType = iota
	FrameCallCode
	FrameDelegateCall
	FrameStaticCall
	FrameCreate
	FrameCreate2
)

// mapOpcodeToFrameType converts the raw EVM opcode byte handed to us by
// OnEnter into the EMES FrameType enum. core/tracing deliberately doesn't
// import core/vm (to avoid an import cycle), so OnEnter's `typ` parameter is
// just the raw opcode byte -- CALL=0xf1, CALLCODE=0xf2, DELEGATECALL=0xf4,
// STATICCALL=0xfa, CREATE=0xf0, CREATE2=0xf5.
func mapOpcodeToFrameType(typ byte) FrameType {
	switch typ {
	case 0xf0:
		return FrameCreate
	case 0xf1:
		return FrameCall
	case 0xf2:
		return FrameCallCode
	case 0xf4:
		return FrameDelegateCall
	case 0xf5:
		return FrameCreate2
	case 0xfa:
		return FrameStaticCall
	default:
		return FrameCall // non-conforming per EMES-V1-RC3 Extensibility Guardrail; caller should flag
	}
}

type BlockStartEvent struct {
	Sequence   uint64
	EMESVersion string
	Number     uint64
	Timestamp  uint64
}

type TransactionStartEvent struct {
	Sequence uint64
	TxIndex  uint64
	Hash     [32]byte
	From     [20]byte
	To       *[20]byte
	Value    [32]byte
	GasLimit uint64
	GasPrice [32]byte
	Nonce    uint64
}

type FrameEnterEvent struct {
	Sequence      uint64
	FrameID       uint64
	ParentFrameID uint64
	Depth         uint64
	Type          FrameType
	From          [20]byte
	To            [20]byte
}

type FrameExitEvent struct {
	Sequence uint64
	FrameID  uint64
	Reverted bool
}

type TransactionEndEvent struct {
	Sequence uint64
	TxIndex  uint64
	Reverted bool
}

type BlockCommitEvent struct {
	Sequence  uint64
	StateRoot [32]byte
}

type BalanceMutationEvent struct {
	Sequence uint64
	FrameID  uint64
	Address  [20]byte
	Before   [32]byte
	After    [32]byte
}

type NonceMutationEvent struct {
	Sequence uint64
	FrameID  uint64
	Address  [20]byte
	Before   uint64
	After    uint64
}

type CodeMutationEvent struct {
	Sequence uint64
	FrameID  uint64
	Address  [20]byte
	Before   [32]byte // code hash, before
	After    [32]byte // code hash, after
}

type StorageMutationEvent struct {
	Sequence uint64
	FrameID  uint64
	Address  [20]byte
	Slot     [32]byte
	Before   [32]byte
	After    [32]byte
}

type AccountCreatedEvent struct {
	Sequence uint64
	FrameID  uint64
	Address  [20]byte
	Creator  [20]byte
}

type SelfDestructEvent struct {
	Sequence    uint64
	FrameID     uint64
	Address     [20]byte
	Beneficiary [20]byte
}

// --- Frame tracking (still our own responsibility -- Geth's hooks give us
// `depth`, not a stable FrameID, so EMES's FrameID/ParentFrameID bookkeeping
// still needs to live in the tracer, same as the original design.) ---

const noParentFrame = ^uint64(0) // 0xFFFFFFFFFFFFFFFF sentinel per EMES-V1-RC3 §2.2

type frameCtx struct {
	frameID       uint64
	parentFrameID uint64
	depth         uint64
	addr          common.Address // address of this frame, needed to correlate SELFDESTRUCT
}

// Tracer implements EMES-V1-RC3 collection against go-ethereum's current
// tracing.Hooks live-tracing API.
type Tracer struct {
	globalSequence uint64
	nextFrameID    uint64
	frameStack     []frameCtx

	blockStream []interface{}

	// pendingSelfDestruct correlates the CodeChangeSelfDestruct code-change
	// on the destructing account with the BalanceIncreaseSelfdestruct credit
	// on the beneficiary, since EMES's SelfDestructEvent needs both and Geth
	// emits them as two separate, address-keyed hook calls rather than one
	// combined event. Keyed by FrameID since both halves fire within the
	// same call frame.
	pendingSelfDestruct map[uint64]common.Address
}

func New() *Tracer {
	return &Tracer{
		frameStack:          make([]frameCtx, 0, 8),
		blockStream:         make([]interface{}, 0, 256),
		pendingSelfDestruct: make(map[uint64]common.Address),
	}
}

func (t *Tracer) seq() uint64 {
	s := t.globalSequence
	t.globalSequence++
	// Overflow protection per EMES-V1-RC3 §2.1: in practice a single block
	// will never approach 2^64 events; this check exists so the tracer fails
	// loudly instead of silently wrapping if it ever somehow did.
	if t.globalSequence == 0 {
		panic("kaysentinel: EMES sequence counter overflowed uint64 -- aborting per §2.1")
	}
	return s
}

func to32(b common.Hash) [32]byte { return b }

func bigTo32(v *big.Int) [32]byte {
	var out [32]byte
	if v == nil {
		return out
	}
	v.FillBytes(out[:]) // big-endian per EMES-V1-RC3 §1.1
	return out
}

// --- Chain/VM event hooks ---

func (t *Tracer) OnBlockStart(ev tracing.BlockEvent) {
	t.blockStream = t.blockStream[:0]
	t.blockStream = append(t.blockStream, BlockStartEvent{
		Sequence:    t.seq(),
		EMESVersion: "EMES-V1",
		Number:      ev.Block.NumberU64(),
		Timestamp:   ev.Block.Time(),
	})
}

func (t *Tracer) OnBlockEnd(err error) {
	// StateRoot is populated via OnStateUpdate (below), not here -- OnBlockEnd
	// only carries an error, not the post-block root.
}

func (t *Tracer) OnStateUpdate(update *tracing.StateUpdate) {
	t.blockStream = append(t.blockStream, BlockCommitEvent{
		Sequence:  t.seq(),
		StateRoot: to32(update.Root),
	})
}

func (t *Tracer) OnTxStart(vmctx *tracing.VMContext, tx *types.Transaction, from common.Address) {
	t.nextFrameID = 0
	t.frameStack = t.frameStack[:0]

	var toPtr *[20]byte
	if to := tx.To(); to != nil {
		var b [20]byte = *to
		toPtr = &b
	}

	t.blockStream = append(t.blockStream, TransactionStartEvent{
		Sequence: t.seq(),
		Hash:     tx.Hash(),
		From:     from,
		To:       toPtr,
		Value:    bigTo32(tx.Value()),
		GasLimit: tx.Gas(),
		GasPrice: bigTo32(tx.GasPrice()),
		Nonce:    tx.Nonce(),
	})
}

func (t *Tracer) OnTxEnd(receipt *types.Receipt, err error) {
	reverted := err != nil || (receipt != nil && receipt.Status == types.ReceiptStatusFailed)
	t.blockStream = append(t.blockStream, TransactionEndEvent{
		Sequence: t.seq(),
		Reverted: reverted,
	})
	t.frameStack = t.frameStack[:0]
	t.pendingSelfDestruct = make(map[uint64]common.Address)
}

func (t *Tracer) OnEnter(depth int, typ byte, from common.Address, to common.Address, input []byte, gas uint64, value *big.Int) {
	var parentID uint64 = noParentFrame
	if len(t.frameStack) > 0 {
		parentID = t.frameStack[len(t.frameStack)-1].frameID
	}
	fc := frameCtx{
		frameID:       t.nextFrameID,
		parentFrameID: parentID,
		depth:         uint64(depth),
		addr:          to,
	}
	t.frameStack = append(t.frameStack, fc)
	t.nextFrameID++

	t.blockStream = append(t.blockStream, FrameEnterEvent{
		Sequence:      t.seq(),
		FrameID:       fc.frameID,
		ParentFrameID: fc.parentFrameID,
		Depth:         fc.depth,
		Type:          mapOpcodeToFrameType(typ),
		From:          from,
		To:            to,
	})
}

func (t *Tracer) OnExit(depth int, output []byte, gasUsed uint64, err error, reverted bool) {
	if len(t.frameStack) == 0 {
		return // defensive; should not happen if OnEnter/OnExit are balanced
	}
	fc := t.frameStack[len(t.frameStack)-1]
	t.frameStack = t.frameStack[:len(t.frameStack)-1]

	t.blockStream = append(t.blockStream, FrameExitEvent{
		Sequence: t.seq(),
		FrameID:  fc.frameID,
		Reverted: reverted,
	})
}

// --- State mutation hooks (Geth now hands us Before/After directly --
// no need to manually diff via StateDB.GetState the way the old
// CaptureState-based design had to.) ---

func (t *Tracer) currentFrameID() uint64 {
	if len(t.frameStack) == 0 {
		return noParentFrame
	}
	return t.frameStack[len(t.frameStack)-1].frameID
}

func (t *Tracer) OnBalanceChange(addr common.Address, prev, new *big.Int, reason tracing.BalanceChangeReason) {
	frameID := t.currentFrameID()
	t.blockStream = append(t.blockStream, BalanceMutationEvent{
		Sequence: t.seq(),
		FrameID:  frameID,
		Address:  addr,
		Before:   bigTo32(prev),
		After:    bigTo32(new),
	})

	if reason == tracing.BalanceIncreaseSelfdestruct {
		if destructed, ok := t.pendingSelfDestruct[frameID]; ok {
			t.blockStream = append(t.blockStream, SelfDestructEvent{
				Sequence:    t.seq(),
				FrameID:     frameID,
				Address:     destructed,
				Beneficiary: addr,
			})
			delete(t.pendingSelfDestruct, frameID)
		}
	}
}

func (t *Tracer) OnNonceChangeV2(addr common.Address, prev, new uint64, reason tracing.NonceChangeReason) {
	frameID := t.currentFrameID()
	t.blockStream = append(t.blockStream, NonceMutationEvent{
		Sequence: t.seq(),
		FrameID:  frameID,
		Address:  addr,
		Before:   prev,
		After:    new,
	})

	if reason == tracing.NonceChangeContractCreator {
		// A contract-creating nonce bump on `addr` (the creator) doesn't by
		// itself tell us the new contract's address -- that comes from the
		// paired FrameEnterEvent (Create/Create2) which already carries `To`.
		// Left as a hook point for AccountCreatedEvent synthesis; see
		// docs/emes_profile.md open item on account-creation correlation.
		_ = addr
	}
}

func (t *Tracer) OnCodeChangeV2(addr common.Address, prevCodeHash common.Hash, prevCode []byte, codeHash common.Hash, code []byte, reason tracing.CodeChangeReason) {
	frameID := t.currentFrameID()
	t.blockStream = append(t.blockStream, CodeMutationEvent{
		Sequence: t.seq(),
		FrameID:  frameID,
		Address:  addr,
		Before:   to32(prevCodeHash),
		After:    to32(codeHash),
	})

	if reason == tracing.CodeChangeSelfDestruct {
		t.pendingSelfDestruct[frameID] = addr
	}
}

func (t *Tracer) OnStorageChange(addr common.Address, slot common.Hash, prev, new common.Hash) {
	t.blockStream = append(t.blockStream, StorageMutationEvent{
		Sequence: t.seq(),
		FrameID:  t.currentFrameID(),
		Address:  addr,
		Slot:     to32(slot),
		Before:   to32(prev),
		After:    to32(new),
	})
}

// Hooks returns the *tracing.Hooks struct Geth's live tracing registry
// expects. This is the entry point a `LiveDirectory.Register`-style
// constructor should return.
func (t *Tracer) Hooks() *tracing.Hooks {
	return &tracing.Hooks{
		OnBlockStart:    t.OnBlockStart,
		OnBlockEnd:      t.OnBlockEnd,
		OnStateUpdate:   t.OnStateUpdate,
		OnTxStart:       t.OnTxStart,
		OnTxEnd:         t.OnTxEnd,
		OnEnter:         t.OnEnter,
		OnExit:          t.OnExit,
		OnBalanceChange: t.OnBalanceChange,
		OnNonceChangeV2: t.OnNonceChangeV2,
		OnCodeChangeV2:  t.OnCodeChangeV2,
		OnStorageChange: t.OnStorageChange,
	}
}

// newLiveTracer is the shape go-ethereum's live tracing registry expects:
// func(json.RawMessage) (*tracing.Hooks, error). Wire this up via
// eth/tracers.LiveDirectory.Register("kaysentinel", newLiveTracer) in a
// client build; see docs/emes_profile.md for the remaining integration step.
func newLiveTracer(cfg json.RawMessage) (*tracing.Hooks, error) {
	return New().Hooks(), nil
}
