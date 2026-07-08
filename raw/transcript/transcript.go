// Package transcript implements a flat, low-allocation in-process buffer
// for raw execution trace events. This version replaces an earlier
// tagged-union design (nativeEvent/tracePayload + StreamProjection +
// ComputeDeterministicBinaryHashV1) with a single flat RawEvent struct, per
// an explicit design decision to prioritize hot-path allocation cost over
// the immutable-projection and canonical-hash features the previous
// version had. See raw/transcript/README.md for that tradeoff record.
//
// The originally pasted version of this file only defined
// KindTxStart/KindTxEnd/KindEnter/KindExit/KindFault/KindBalanceChange and
// only implemented AppendEnter/AppendExit. That's not enough to capture
// what EMES-V1 (docs/emes_profile.md) needs to reconstruct an SSR --
// nonce/code/storage mutations and self-destruct/account-creation were
// missing entirely. This version adds them, following the same flat,
// single-struct design as the rest of the file.
package transcript

import (
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/tracing"
	"github.com/holiman/uint256"
)

type EventKind uint16

const (
	KindTxStart EventKind = iota
	KindTxEnd
	KindEnter
	KindExit
	KindFault
	KindBalanceChange
	KindNonceChange
	KindCodeChange
	KindStorageChange
	KindSelfDestruct
	KindAccountCreated
)

// RawEvent is intentionally a single flat struct (not a tagged union) so
// appending never allocates anything beyond growing the backing slice --
// every event, regardless of Kind, is one value copy into that slice.
// Fields not relevant to a given Kind are left at their zero value.
type RawEvent struct {
	Sequence uint64
	Kind     EventKind

	// Enter/Exit/Fault
	Opcode  byte // raw EVM opcode byte, e.g. 0xF1 = CALL
	Depth   int
	From    common.Address
	To      common.Address
	Value   uint256.Int
	GasUsed uint64

	// TxStart
	GasLimit uint64

	// Exit/TxEnd
	Reverted bool
	ErrType  string // kept as a string despite the file's own "zero-allocation"
	// framing in the original pasted design -- ErrorSnapshot-style
	// structured errors were dropped along with the rest of the tagged-union
	// design in this rewrite. Flagged here rather than silently left
	// inconsistent with the stated goal.

	// BalanceChange
	BalanceBefore uint256.Int
	BalanceAfter  uint256.Int
	BalanceReason tracing.BalanceChangeReason

	// NonceChange
	NonceBefore uint64
	NonceAfter  uint64

	// CodeChange
	CodeHashBefore common.Hash
	CodeHashAfter  common.Hash

	// StorageChange
	Slot          common.Hash
	StorageBefore common.Hash
	StorageAfter  common.Hash

	// SelfDestruct
	Beneficiary common.Address

	// AccountCreated
	Creator common.Address
}

type ExecutionTranscript struct {
	seq    uint64 // single-threaded per block-worker execution model
	events []RawEvent
}

func NewExecutionTranscript() *ExecutionTranscript {
	return &ExecutionTranscript{events: make([]RawEvent, 0, 256)}
}

func (t *ExecutionTranscript) Len() int { return len(t.events) }

func (t *ExecutionTranscript) append(ev RawEvent) {
	ev.Sequence = t.seq
	t.events = append(t.events, ev)
	t.seq++
}

func (t *ExecutionTranscript) AppendTxStart(gasLimit uint64) {
	t.append(RawEvent{Kind: KindTxStart, GasLimit: gasLimit})
}

func (t *ExecutionTranscript) AppendTxEnd(gasUsed uint64, reverted bool, errType string) {
	t.append(RawEvent{Kind: KindTxEnd, GasUsed: gasUsed, Reverted: reverted, ErrType: errType})
}

func (t *ExecutionTranscript) AppendEnter(depth int, op byte, from, to common.Address, gas uint64, val *uint256.Int) {
	var v uint256.Int
	if val != nil {
		v = *val
	}
	t.append(RawEvent{
		Kind: KindEnter, Opcode: op, Depth: depth, From: from, To: to, Value: v, GasUsed: gas,
	})
}

func (t *ExecutionTranscript) AppendExit(depth int, gasUsed uint64, reverted bool, errType string) {
	t.append(RawEvent{Kind: KindExit, Depth: depth, GasUsed: gasUsed, Reverted: reverted, ErrType: errType})
}

func (t *ExecutionTranscript) AppendFault(depth int, op byte, gasUsed uint64, errType string) {
	t.append(RawEvent{Kind: KindFault, Depth: depth, Opcode: op, GasUsed: gasUsed, ErrType: errType})
}

func (t *ExecutionTranscript) AppendBalanceChange(addr common.Address, before, after *uint256.Int, reason tracing.BalanceChangeReason) {
	var b, a uint256.Int
	if before != nil {
		b = *before
	}
	if after != nil {
		a = *after
	}
	t.append(RawEvent{Kind: KindBalanceChange, From: addr, BalanceBefore: b, BalanceAfter: a, BalanceReason: reason})
}

func (t *ExecutionTranscript) AppendNonceChange(addr common.Address, before, after uint64) {
	t.append(RawEvent{Kind: KindNonceChange, From: addr, NonceBefore: before, NonceAfter: after})
}

func (t *ExecutionTranscript) AppendCodeChange(addr common.Address, before, after common.Hash) {
	t.append(RawEvent{Kind: KindCodeChange, From: addr, CodeHashBefore: before, CodeHashAfter: after})
}

func (t *ExecutionTranscript) AppendStorageChange(addr common.Address, slot, before, after common.Hash) {
	t.append(RawEvent{Kind: KindStorageChange, From: addr, Slot: slot, StorageBefore: before, StorageAfter: after})
}

func (t *ExecutionTranscript) AppendSelfDestruct(addr, beneficiary common.Address) {
	t.append(RawEvent{Kind: KindSelfDestruct, From: addr, Beneficiary: beneficiary})
}

func (t *ExecutionTranscript) AppendAccountCreated(addr, creator common.Address) {
	t.append(RawEvent{Kind: KindAccountCreated, From: addr, Creator: creator})
}

func (t *ExecutionTranscript) StreamRaw(receiver func(RawEvent) error) error {
	for _, ev := range t.events {
		if err := receiver(ev); err != nil {
			return err
		}
	}
	return nil
}
