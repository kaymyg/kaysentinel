package validation

import (
	"strings"
	"testing"

	"kaysentinel/emes"
)

func seq(base *uint64) uint64 {
	v := *base
	*base += 1
	return v
}

// buildStream wraps a slice of events with a BlockStartEvent/BlockCommitEvent
// pair and assigns strictly increasing sequence numbers, since every real
// test case needs both (Gate 1's block-encapsulation check runs first,
// unconditionally).
func buildStream(inner ...emes.MutableEvent) []emes.Event {
	var s uint64
	stream := make([]emes.Event, 0, len(inner)+2)

	start := &emes.BlockStartEvent{}
	start.SetSequence(seq(&s))
	stream = append(stream, start)

	for _, e := range inner {
		e.SetSequence(seq(&s))
		stream = append(stream, e)
	}

	commit := &emes.BlockCommitEvent{}
	commit.SetSequence(seq(&s))
	stream = append(stream, commit)

	return stream
}

func TestT1_RootFrameCommit_TransactionCommit_Passes(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: false},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	if err := VerifyGate1Invariants(stream); err != nil {
		t.Fatalf("expected pass, got error: %v", err)
	}
}

func TestT1_RootFrameRevert_TransactionRevert_Passes(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: true},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: true},
	)
	if err := VerifyGate1Invariants(stream); err != nil {
		t.Fatalf("expected pass, got error: %v", err)
	}
}

func TestT1_RootCommitButTransactionRevert_Fails(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: false},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: true},
	)
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected a t1-consistency error, got nil")
	}
	if !strings.Contains(err.Error(), "t1-consistency") {
		t.Fatalf("expected t1-consistency rule violation, got: %v", err)
	}
}

func TestT1_RootRevertButTransactionCommits_Fails(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: true},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected a t1-consistency error, got nil")
	}
	if !strings.Contains(err.Error(), "t1-consistency") {
		t.Fatalf("expected t1-consistency rule violation, got: %v", err)
	}
}

// TestT1_ZeroFrameTransaction_IsExempt proves the T0-retraction finding is
// correctly encoded: a transaction with no root frame at all (e.g. a
// pre-execution validation failure, per core/state_transition.go's preCheck)
// must NOT be rejected by T1, regardless of its Reverted value.
func TestT1_ZeroFrameTransaction_IsExempt(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: true},
	)
	if err := VerifyGate1Invariants(stream); err != nil {
		t.Fatalf("expected zero-frame transaction to pass T1 unconditionally, got error: %v", err)
	}

	streamCommitted := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	if err := VerifyGate1Invariants(streamCommitted); err != nil {
		t.Fatalf("expected zero-frame transaction (Reverted=false) to pass T1, got error: %v", err)
	}
}

// TestT1_NestedFrames_OnlyRootMatters proves a child frame's own Reverted
// status has no bearing on T1 -- only the root frame's does. This models
// the real "CALL returning 0, caller catches it and continues" case.
func TestT1_NestedFrames_ChildRevertDoesNotAffectRoot(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameEnterEvent{FrameID: 1, ParentFrameID: 0, Depth: 1},
		&emes.FrameExitEvent{FrameID: 1, Reverted: true}, // child reverts...
		&emes.FrameExitEvent{FrameID: 0, Reverted: false}, // ...but root still commits
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	if err := VerifyGate1Invariants(stream); err != nil {
		t.Fatalf("expected pass (only root frame's Reverted matters for T1), got error: %v", err)
	}
}

// TestT1_MultipleRootFrames_Fails proves the "exactly one root frame" half
// of T1's constraint: a transaction whose stack drains to empty and then
// pushes a second root-looking frame must be rejected, not silently allowed
// through with only the second root's Reverted value checked.
func TestT1_MultipleRootFrames_Fails(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: false},
		&emes.FrameEnterEvent{FrameID: 1, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 1, Reverted: false},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected a t1-consistency error for multiple root frames, got nil")
	}
	if !strings.Contains(err.Error(), "t1-consistency") {
		t.Fatalf("expected t1-consistency rule violation, got: %v", err)
	}
}

// TestT1_StateResetsBetweenTransactions proves root-frame tracking state
// doesn't leak from one transaction to the next within the same block.
func TestT1_StateResetsBetweenTransactions(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.FrameExitEvent{FrameID: 0, Reverted: true},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: true},
		// second transaction: zero frames, must be exempt regardless of
		// what the first transaction's root frame did.
		&emes.TransactionStartEvent{TxIndex: 1},
		&emes.TransactionEndEvent{TxIndex: 1, Reverted: false},
	)
	if err := VerifyGate1Invariants(stream); err != nil {
		t.Fatalf("expected pass, got error: %v", err)
	}
}

// Pre-existing structural checks (unchanged by the T1 patch) still pass --
// a minimal regression check that this patch didn't alter prior behavior.
func TestPreExisting_EmptyStreamRejected(t *testing.T) {
	err := VerifyGate1Invariants(nil)
	if err == nil {
		t.Fatal("expected error for empty stream")
	}
}

func TestPreExisting_UnbalancedFrameRejected(t *testing.T) {
	stream := buildStream(
		&emes.TransactionStartEvent{TxIndex: 0},
		&emes.FrameEnterEvent{FrameID: 0, ParentFrameID: ^uint64(0), Depth: 0},
		&emes.TransactionEndEvent{TxIndex: 0, Reverted: false},
	)
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected frame-balance error for unclosed frame")
	}
	if !strings.Contains(err.Error(), "frame-balance") {
		t.Fatalf("expected frame-balance rule violation, got: %v", err)
	}
}

// --- Block encapsulation (EMES-V1 section 2) --------------------------------
//
// VerifyGate1Invariants documents "exactly one BlockStartEvent at Sequence 0,
// exactly one terminal BlockCommitEvent", but for a long time only the first
// and last elements of the stream were actually checked -- extra markers in
// the interior passed silently. These three cases pin that down so the hole
// cannot reopen.

func TestBlockEncapsulation_DuplicateBlockStartRejected(t *testing.T) {
	// [BlockStart, BlockStart, BlockCommit] -- the interior one is illegal.
	stream := buildStream(&emes.BlockStartEvent{})
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected error for a second BlockStartEvent")
	}
	if !strings.Contains(err.Error(), "block-encapsulation") {
		t.Fatalf("expected block-encapsulation rule violation, got: %v", err)
	}
}

func TestBlockEncapsulation_DuplicateBlockCommitRejected(t *testing.T) {
	// [BlockStart, BlockCommit, BlockCommit] -- only the terminal one is legal.
	stream := buildStream(&emes.BlockCommitEvent{})
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected error for a non-terminal BlockCommitEvent")
	}
	if !strings.Contains(err.Error(), "block-encapsulation") {
		t.Fatalf("expected block-encapsulation rule violation, got: %v", err)
	}
}

func TestBlockEncapsulation_CommitWithOpenTransactionRejected(t *testing.T) {
	// [BlockStart, TxStart, BlockCommit] -- the block closes mid-transaction,
	// which is what a truncated or partially suppressed stream looks like.
	stream := buildStream(&emes.TransactionStartEvent{TxIndex: 0})
	err := VerifyGate1Invariants(stream)
	if err == nil {
		t.Fatal("expected error for BlockCommitEvent while a transaction is open")
	}
	if !strings.Contains(err.Error(), "tx-encapsulation") {
		t.Fatalf("expected tx-encapsulation rule violation, got: %v", err)
	}
}