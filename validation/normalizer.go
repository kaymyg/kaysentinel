// Package validation (normalizer) maps raw transcript events back to
// human-readable semantic labels for differential comparison against a
// legacy tracer's output.
package validation

import (
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/vm"
	"github.com/holiman/uint256"

	"kaysentinel/raw/transcript"
)

type SemanticEvent struct {
	Sequence uint64
	Opcode   string
	Depth    int
	From     common.Address
	To       common.Address
	Value    uint256.Int
	GasUsed  uint64
	Reverted bool
	ErrType  string
}

type EventNormalizer struct{}

func NewEventNormalizer() *EventNormalizer { return &EventNormalizer{} }

// NormalizeRaw maps a transcript.RawEvent to a SemanticEvent. Note this
// only surfaces the fields SemanticEvent actually has (Sequence, Opcode,
// Depth, From, To, Value, GasUsed, Reverted, ErrType) -- kind-specific
// detail like storage slot before/after, nonce before/after, or a
// self-destruct beneficiary isn't represented in SemanticEvent as designed
// here, so a differential check on those specifics isn't possible without
// extending SemanticEvent. Flagged rather than silently dropped.
func (n *EventNormalizer) NormalizeRaw(raw transcript.RawEvent) SemanticEvent {
	opString := "UNKNOWN"
	switch raw.Kind {
	case transcript.KindTxStart:
		opString = "TX_START"
	case transcript.KindTxEnd:
		opString = "TX_END"
	case transcript.KindEnter:
		opString = vm.OpCode(raw.Opcode).String() // preserves DELEGATECALL/STATICCALL variants intact
	case transcript.KindExit:
		opString = "EXIT"
	case transcript.KindFault:
		opString = "FAULT"
	case transcript.KindBalanceChange:
		opString = "BALANCE_CHANGE"
	case transcript.KindNonceChange:
		opString = "NONCE_CHANGE"
	case transcript.KindCodeChange:
		opString = "CODE_CHANGE"
	case transcript.KindStorageChange:
		opString = "STORAGE_CHANGE"
	case transcript.KindSelfDestruct:
		opString = "SELFDESTRUCT"
	case transcript.KindAccountCreated:
		opString = "ACCOUNT_CREATED"
	}

	return SemanticEvent{
		Sequence: raw.Sequence,
		Opcode:   opString,
		Depth:    raw.Depth,
		From:     raw.From,
		To:       raw.To,
		Value:    raw.Value,
		GasUsed:  raw.GasUsed,
		Reverted: raw.Reverted,
		ErrType:  raw.ErrType,
	}
}
