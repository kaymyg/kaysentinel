// Package validation implements Gate 1 of the conformance architecture:
// structural verification of an EMES-V1 event stream, independent of what
// the mutations actually mean semantically (that's Gate 2, which needs a
// real state-reconstruction engine and doesn't exist yet).
package validation

import (
	"fmt"

	"kaysentinel/emes"
)

// Gate1Error reports a specific structural invariant violation, including
// which event index it was found at, so a caller can point at the exact
// offending event rather than just getting a generic failure.
type Gate1Error struct {
	Index int
	Rule  string
	Msg   string
}

func (e *Gate1Error) Error() string {
	return fmt.Sprintf("gate1: event[%d]: %s: %s", e.Index, e.Rule, e.Msg)
}

// VerifyGate1Invariants checks the structural (not semantic) invariants
// from EMES-V1 §2 and §4:
//   - exactly one BlockStartEvent at Sequence 0, exactly one terminal
//     BlockCommitEvent
//   - Sequence values strictly increasing across the whole stream
//   - every TransactionStartEvent is matched by exactly one
//     TransactionEndEvent before another TransactionStartEvent begins
//   - FrameEnter/FrameExit are stack-balanced within each transaction, and
//     FrameID 0's ParentFrameID is the sentinel value
//
// It deliberately does NOT check whether the mutation events represent a
// state-consistent execution (e.g. that a StorageMutationEvent's Before
// matches the previous After for that slot) -- that's semantic replay
// (Gate 2), out of scope here.
func VerifyGate1Invariants(stream []emes.Event) error {
	if len(stream) == 0 {
		return &Gate1Error{Index: -1, Rule: "block-encapsulation", Msg: "empty event stream"}
	}

	first, ok := stream[0].(*emes.BlockStartEvent)
	if !ok {
		return &Gate1Error{Index: 0, Rule: "block-encapsulation", Msg: "stream must begin with BlockStartEvent"}
	}
	if first.SequenceNum() != 0 {
		return &Gate1Error{Index: 0, Rule: "block-encapsulation", Msg: "BlockStartEvent must have Sequence 0"}
	}

	last := stream[len(stream)-1]
	if _, ok := last.(*emes.BlockCommitEvent); !ok {
		return &Gate1Error{Index: len(stream) - 1, Rule: "block-encapsulation", Msg: "stream must end with BlockCommitEvent"}
	}

	var prevSeq uint64
	var seqInitialized bool
	var txOpen bool
	frameStack := make([]uint64, 0, 8)

	// Invariant T1 (docs/emes/003-validation-invariants.md) state, reset per
	// transaction: how many times this transaction pushed a frame onto an
	// empty stack (a genuine root frame -- including a rejected "root again"
	// re-entry after the stack already drained once), and the Reverted value
	// the root frame's own FrameExitEvent carried.
	var rootFrameCount int
	var rootFrameReverted bool

	for i, e := range stream {
		seq := e.SequenceNum()
		if seqInitialized && seq <= prevSeq {
			return &Gate1Error{Index: i, Rule: "sequence-monotonic", Msg: fmt.Sprintf("sequence %d did not strictly increase from %d", seq, prevSeq)}
		}
		prevSeq = seq
		seqInitialized = true

		switch ev := e.(type) {
		case *emes.TransactionStartEvent:
			if txOpen {
				return &Gate1Error{Index: i, Rule: "tx-encapsulation", Msg: "TransactionStartEvent seen while a transaction is already open"}
			}
			txOpen = true
			frameStack = frameStack[:0]
			rootFrameCount = 0
			rootFrameReverted = false

		case *emes.TransactionEndEvent:
			if !txOpen {
				return &Gate1Error{Index: i, Rule: "tx-encapsulation", Msg: "TransactionEndEvent seen with no open transaction"}
			}
			if len(frameStack) != 0 {
				return &Gate1Error{Index: i, Rule: "frame-balance", Msg: fmt.Sprintf("transaction ended with %d unclosed frame(s)", len(frameStack))}
			}
			// Invariant T1 (docs/emes/003-validation-invariants.md): only
			// applies if this transaction had a root frame at all. A
			// zero-frame transaction (e.g. a pre-execution validation
			// failure such as insufficient balance or bad nonce, verified
			// against go-ethereum's core/state_transition.go) is outside
			// T1's domain entirely -- it is not a violation for one not to
			// exist.
			if rootFrameCount > 1 {
				return &Gate1Error{Index: i, Rule: "t1-consistency", Msg: fmt.Sprintf("transaction contains %d root frames; exactly one is required when any root frame is present", rootFrameCount)}
			}
			if rootFrameCount == 1 && ev.Reverted != rootFrameReverted {
				return &Gate1Error{Index: i, Rule: "t1-consistency", Msg: fmt.Sprintf("TransactionEndEvent.Reverted (%v) does not match root FrameExitEvent.Reverted (%v)", ev.Reverted, rootFrameReverted)}
			}
			txOpen = false

		case *emes.FrameEnterEvent:
			if !txOpen {
				return &Gate1Error{Index: i, Rule: "tx-encapsulation", Msg: "FrameEnterEvent seen outside any transaction"}
			}
			if len(frameStack) == 0 {
				if ev.ParentFrameID != ^uint64(0) {
					return &Gate1Error{Index: i, Rule: "frame-topology", Msg: "FrameID 0's ParentFrameID must be the 0xFFFF...FFFF sentinel"}
				}
				rootFrameCount++
			} else if ev.ParentFrameID != frameStack[len(frameStack)-1] {
				return &Gate1Error{Index: i, Rule: "frame-topology", Msg: "ParentFrameID does not match the currently open parent frame"}
			}
			frameStack = append(frameStack, ev.FrameID)

		case *emes.FrameExitEvent:
			if !txOpen {
				return &Gate1Error{Index: i, Rule: "tx-encapsulation", Msg: "FrameExitEvent seen outside any transaction"}
			}
			if len(frameStack) == 0 {
				return &Gate1Error{Index: i, Rule: "frame-balance", Msg: "FrameExitEvent with no matching open frame"}
			}
			top := frameStack[len(frameStack)-1]
			if top != ev.FrameID {
				return &Gate1Error{Index: i, Rule: "frame-balance", Msg: fmt.Sprintf("FrameExitEvent for frame %d does not match innermost open frame %d", ev.FrameID, top)}
			}
			if len(frameStack) == 1 {
				// This exit drains the stack back to empty -- it's closing
				// the root frame this transaction most recently opened.
				rootFrameReverted = ev.Reverted
			}
			frameStack = frameStack[:len(frameStack)-1]
		}
	}

	if txOpen {
		return &Gate1Error{Index: len(stream) - 1, Rule: "tx-encapsulation", Msg: "stream ended with an open transaction (missing TransactionEndEvent)"}
	}

	return nil
}
