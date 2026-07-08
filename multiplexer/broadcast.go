// Package multiplexer fans a single go-ethereum tracing.Hooks callback out
// to multiple HookSink implementations, isolating a panic in one sink so it
// can't take down the others (or the host EVM) unless strictMode is set.
package multiplexer

import (
	"math/big"
	"runtime/debug"
	"sync"
	"sync/atomic"
	"time"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/tracing"
	"github.com/ethereum/go-ethereum/core/types"
)

type PanicReport struct {
	SinkName   string
	Callback   string
	Timestamp  time.Time
	StackTrace []byte
}

// HookSink signatures below are checked against the real
// github.com/ethereum/go-ethereum/core/tracing hook types. The originally
// pasted version had three mismatches, confirmed by grepping the real
// core/tracing/hooks.go source rather than assumed:
//   - OnTxStart was missing the `from common.Address` parameter that
//     TxStartHook requires, and used a value tracing.VMContext instead of
//     the required pointer *tracing.VMContext.
//   - OnEnter's `value` parameter was typed *uint256.Int; EnterHook
//     requires *big.Int.
//   - OnFault didn't match FaultHook at all: the real signature is
//     func(pc uint64, op byte, gas, cost uint64, scope OpContext, depth int,
//     err error) -- missing the `scope OpContext` parameter entirely, and
//     in a different parameter order (pasted had `depth` first; real hook
//     has it second-to-last).
type HookSink interface {
	Name() string
	OnTxStart(vm *tracing.VMContext, tx *types.Transaction, from common.Address)
	OnTxEnd(receipt *types.Receipt, err error)
	OnEnter(depth int, typ byte, from, to common.Address, input []byte, gas uint64, value *big.Int)
	OnExit(depth int, output []byte, gasUsed uint64, err error, reverted bool)
	OnFault(pc uint64, op byte, gas, cost uint64, scope tracing.OpContext, depth int, err error)
	OnBalanceChange(addr common.Address, prev, new *big.Int, reason tracing.BalanceChangeReason)
}

type BroadcastSink struct {
	active       atomic.Value // stores immutable []HookSink
	strictMode   bool
	mu           sync.Mutex
	panicReports []PanicReport
}

func NewBroadcastSink(strictMode bool, initialSinks ...HookSink) *BroadcastSink {
	b := &BroadcastSink{strictMode: strictMode}
	sinksCopy := make([]HookSink, len(initialSinks))
	copy(sinksCopy, initialSinks)
	b.active.Store(sinksCopy)
	return b
}

func (b *BroadcastSink) PanicReports() []PanicReport {
	b.mu.Lock()
	defer b.mu.Unlock()
	out := make([]PanicReport, len(b.panicReports))
	copy(out, b.panicReports)
	return out
}

func (b *BroadcastSink) recordAndEvacuate(sinkName, callback string, r any) {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.panicReports = append(b.panicReports, PanicReport{
		SinkName:   sinkName,
		Callback:   callback,
		StackTrace: debug.Stack(),
		Timestamp:  time.Now().UTC(),
	})

	if current, ok := b.active.Load().([]HookSink); ok {
		next := make([]HookSink, 0, len(current))
		for _, s := range current {
			if s.Name() != sinkName {
				next = append(next, s)
			}
		}
		b.active.Store(next)
	}
}

// guard wraps a single sink call with panic isolation: on panic, records a
// PanicReport and evicts the offending sink from future dispatch, then
// (only in strictMode) rethrows the original panic value untouched via
// `panic(r)`, preserving the original call-site context rather than
// wrapping it in a new error.
func (b *BroadcastSink) guard(sink HookSink, callback string, fn func()) {
	defer func() {
		if r := recover(); r != nil {
			b.recordAndEvacuate(sink.Name(), callback, r)
			if b.strictMode {
				panic(r)
			}
		}
	}()
	fn()
}

func (b *BroadcastSink) sinks() []HookSink {
	return b.active.Load().([]HookSink)
}

// Hooks returns the *tracing.Hooks struct that fans every call out to all
// currently-active sinks, isolating panics per-sink.
func (b *BroadcastSink) Hooks() *tracing.Hooks {
	return &tracing.Hooks{
		OnTxStart: func(vm *tracing.VMContext, tx *types.Transaction, from common.Address) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnTxStart", func() { s.OnTxStart(vm, tx, from) })
			}
		},
		OnTxEnd: func(receipt *types.Receipt, err error) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnTxEnd", func() { s.OnTxEnd(receipt, err) })
			}
		},
		OnEnter: func(depth int, typ byte, from, to common.Address, input []byte, gas uint64, value *big.Int) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnEnter", func() { s.OnEnter(depth, typ, from, to, input, gas, value) })
			}
		},
		OnExit: func(depth int, output []byte, gasUsed uint64, err error, reverted bool) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnExit", func() { s.OnExit(depth, output, gasUsed, err, reverted) })
			}
		},
		OnFault: func(pc uint64, op byte, gas, cost uint64, scope tracing.OpContext, depth int, err error) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnFault", func() { s.OnFault(pc, op, gas, cost, scope, depth, err) })
			}
		},
		OnBalanceChange: func(addr common.Address, prev, new *big.Int, reason tracing.BalanceChangeReason) {
			for _, s := range b.sinks() {
				s := s
				b.guard(s, "OnBalanceChange", func() { s.OnBalanceChange(addr, prev, new, reason) })
			}
		},
	}
}
