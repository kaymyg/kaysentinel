// Package tracer implements EMES-V1 event collection against go-ethereum's
// current live-tracing API (core/tracing.Hooks). See docs/emes_profile.md
// for why this targets Hooks rather than the removed vm.EVMLogger
// interface, and docs/emes_profile.md's changelog for why this version
// moved the wire event types out to package emes and added structured
// error reporting + EnvironmentDescriptor support.
package tracer

import (
	"encoding/json"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/tracing"
	"github.com/ethereum/go-ethereum/core/types"

	"kaysentinel/emes"
)

const noParentFrame = ^uint64(0) // 0xFFFFFFFFFFFFFFFF sentinel per EMES-V1 §2.2

// mapOpcodeToFrameType converts the raw EVM opcode byte OnEnter hands us
// into the EMES FrameType enum. core/tracing deliberately doesn't import
// core/vm (avoids an import cycle), so `typ` here is the raw opcode byte:
// CALL=0xf1, CALLCODE=0xf2, DELEGATECALL=0xf4, STATICCALL=0xfa, CREATE=0xf0,
// CREATE2=0xf5.
func mapOpcodeToFrameType(typ byte) emes.FrameType {
	switch typ {
	case 0xf0:
		return emes.FrameCreate
	case 0xf1:
		return emes.FrameCall
	case 0xf2:
		return emes.FrameCallCode
	case 0xf4:
		return emes.FrameDelegateCall
	case 0xf5:
		return emes.FrameCreate2
	case 0xfa:
		return emes.FrameStaticCall
	default:
		return emes.FrameCall // non-conforming per EMES-V1 Extensibility Guardrail
	}
}

type frameCtx struct {
	frameID       uint64
	parentFrameID uint64
	depth         uint64
	addr          common.Address
}

// Tracer implements EMES-V1 collection against go-ethereum's
// core/tracing.Hooks live-tracing API.
type Tracer struct {
	env emes.EnvironmentDescriptor

	globalSequence uint64
	nextFrameID    uint64
	frameStack     []frameCtx

	events   []emes.Event
	errorLog []emes.InternalTracerError

	pendingSelfDestruct map[uint64]common.Address

	// post-state root of the block being traced, captured at OnBlockStart
	blockRoot emes.Hash
}

func New(env emes.EnvironmentDescriptor) *Tracer {
	return &Tracer{
		env:                 env,
		frameStack:          make([]frameCtx, 0, 8),
		events:              make([]emes.Event, 0, 256),
		errorLog:            make([]emes.InternalTracerError, 0),
		pendingSelfDestruct: make(map[uint64]common.Address),
	}
}

// Events returns the collected EMES-V1 stream for the current/most recent
// block.
func (t *Tracer) Events() []emes.Event { return t.events }

// ErrorLog returns structured diagnostics for any internal anomalies the
// tracer observed (e.g. an ExitHook with no matching EnterHook). These are
// bugs in the client adapter or a hook-ordering assumption that didn't
// hold, not normal execution outcomes -- they belong in
// FixtureEnvelope.Errors, not silently dropped.
func (t *Tracer) ErrorLog() []emes.InternalTracerError { return t.errorLog }

func (t *Tracer) logError(code emes.InternalErrorCode, frameID uint64, msg string) {
	t.errorLog = append(t.errorLog, emes.InternalTracerError{
		Code:     code,
		FrameID:  frameID,
		Sequence: t.globalSequence,
		Message:  msg,
	})
}

// emit assigns the next sequence number and appends e to the stream. Takes
// emes.MutableEvent (not emes.Event) specifically so the compiler enforces
// that every emitted event actually has a settable sequence -- you cannot
// accidentally emit something that can't carry a sequence number.
func (t *Tracer) emit(e emes.MutableEvent) {
	e.SetSequence(t.globalSequence)
	t.globalSequence++
	if t.globalSequence == 0 {
		t.logError(emes.ErrSequenceOverflow, t.currentFrameID(), "EMES sequence counter overflowed uint64")
		panic("kaysentinel: EMES sequence counter overflowed uint64 -- aborting per EMES-V1 §2.1")
	}
	t.events = append(t.events, e)
}

func (t *Tracer) currentFrameID() uint64 {
	if len(t.frameStack) == 0 {
		return noParentFrame
	}
	return t.frameStack[len(t.frameStack)-1].frameID
}

func to32(b common.Hash) emes.Hash         { return emes.Hash(b) }
func addr20(a common.Address) emes.Address { return emes.Address(a) }

// --- Chain/VM hooks ---------------------------------------------------------

func (t *Tracer) OnBlockStart(ev tracing.BlockEvent) {
	t.events = t.events[:0]
	t.errorLog = t.errorLog[:0]
	t.blockRoot = to32(ev.Block.Root())
	e := &emes.BlockStartEvent{
		EMESVersion: "EMES-V1",
		Number:      ev.Block.NumberU64(),
		Timestamp:   ev.Block.Time(),
	}
	t.emit(e)
}

func (t *Tracer) OnBlockEnd(err error) {
	// go-ethereum's core/tracing exposes no OnStateUpdate hook and no
	// StateUpdate type, so the post-state root is taken from the block
	// header captured at OnBlockStart. Emitting here also guarantees
	// BlockCommit is the terminal event, which Gate 1 requires.
	e := &emes.BlockCommitEvent{StateRoot: t.blockRoot}
	t.emit(e)
}

func (t *Tracer) OnTxStart(vmctx *tracing.VMContext, tx *types.Transaction, from common.Address) {
	t.nextFrameID = 0
	t.frameStack = t.frameStack[:0]

	var toPtr *emes.Address
	if to := tx.To(); to != nil {
		a := addr20(*to)
		toPtr = &a
	}

	e := &emes.TransactionStartEvent{
		Hash:     emes.Hash(tx.Hash()),
		From:     addr20(from),
		To:       toPtr,
		Value:    emes.BigToHash(tx.Value()),
		GasLimit: tx.Gas(),
		GasPrice: emes.BigToHash(tx.GasPrice()),
		Nonce:    tx.Nonce(),
	}
	t.emit(e)
}

func (t *Tracer) OnTxEnd(receipt *types.Receipt, err error) {
	reverted := err != nil || (receipt != nil && receipt.Status == types.ReceiptStatusFailed)
	e := &emes.TransactionEndEvent{Reverted: reverted}
	t.emit(e)

	if len(t.frameStack) != 0 {
		t.logError(emes.ErrUnexpectedExit, t.currentFrameID(),
			"transaction ended with unclosed frames still on the stack")
	}
	t.frameStack = t.frameStack[:0]
	t.pendingSelfDestruct = make(map[uint64]common.Address)
}

func (t *Tracer) OnEnter(depth int, typ byte, from common.Address, to common.Address, input []byte, gas uint64, value *big.Int) {
	parentID := t.currentFrameID()

	frameType := mapOpcodeToFrameType(typ)
	if t.env != nil && t.env.IsPrecompile(addr20(to)) {
		frameType = emes.FramePrecompile
	}

	fc := frameCtx{frameID: t.nextFrameID, parentFrameID: parentID, depth: uint64(depth), addr: to}
	t.frameStack = append(t.frameStack, fc)
	t.nextFrameID++

	e := &emes.FrameEnterEvent{
		FrameID:       fc.frameID,
		ParentFrameID: fc.parentFrameID,
		Depth:         fc.depth,
		Type:          frameType,
		From:          addr20(from),
		To:            addr20(to),
	}
	t.emit(e)
}

func (t *Tracer) OnExit(depth int, output []byte, gasUsed uint64, err error, reverted bool) {
	if len(t.frameStack) == 0 {
		t.logError(emes.ErrEmptyFrameStack, noParentFrame,
			"ExitHook fired with no matching EnterHook on the frame stack")
		return
	}
	fc := t.frameStack[len(t.frameStack)-1]
	t.frameStack = t.frameStack[:len(t.frameStack)-1]

	e := &emes.FrameExitEvent{FrameID: fc.frameID, Reverted: reverted}
	t.emit(e)
}

// --- State mutation hooks ---------------------------------------------------

func (t *Tracer) OnBalanceChange(addr common.Address, prev, new *big.Int, reason tracing.BalanceChangeReason) {
	frameID := t.currentFrameID()
	e := &emes.BalanceMutationEvent{
		FrameID: frameID,
		Address: addr20(addr),
		Before:  emes.BigToHash(prev),
		After:   emes.BigToHash(new),
	}
	t.emit(e)

	if reason == tracing.BalanceIncreaseSelfdestruct {
		if destructed, ok := t.pendingSelfDestruct[frameID]; ok {
			sd := &emes.SelfDestructEvent{FrameID: frameID, Address: addr20(destructed), Beneficiary: addr20(addr)}
			t.emit(sd)
			delete(t.pendingSelfDestruct, frameID)
		}
	}
}

func (t *Tracer) OnNonceChangeV2(addr common.Address, prev, new uint64, reason tracing.NonceChangeReason) {
	e := &emes.NonceMutationEvent{FrameID: t.currentFrameID(), Address: addr20(addr), Before: prev, After: new}
	t.emit(e)
}

func (t *Tracer) OnCodeChangeV2(addr common.Address, prevCodeHash common.Hash, prevCode []byte, codeHash common.Hash, code []byte, reason tracing.CodeChangeReason) {
	frameID := t.currentFrameID()
	e := &emes.CodeMutationEvent{FrameID: frameID, Address: addr20(addr), Before: to32(prevCodeHash), After: to32(codeHash)}
	t.emit(e)

	if reason == tracing.CodeChangeSelfDestruct {
		t.pendingSelfDestruct[frameID] = addr
	}
}

func (t *Tracer) OnStorageChange(addr common.Address, slot common.Hash, prev, new common.Hash) {
	e := &emes.StorageMutationEvent{FrameID: t.currentFrameID(), Address: addr20(addr), Slot: to32(slot), Before: to32(prev), After: to32(new)}
	t.emit(e)
}

// Hooks returns the *tracing.Hooks struct Geth's live tracing registry
// expects.
func (t *Tracer) Hooks() *tracing.Hooks {
	return &tracing.Hooks{
		OnBlockStart:    t.OnBlockStart,
		OnBlockEnd:      t.OnBlockEnd,
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
// func(json.RawMessage) (*tracing.Hooks, error). `cfg` would carry which
// EnvironmentDescriptor to construct in a real integration; passing nil for
// env here since that wiring depends on the host client build.
func newLiveTracer(cfg json.RawMessage) (*tracing.Hooks, error) {
	return New(nil).Hooks(), nil
}
