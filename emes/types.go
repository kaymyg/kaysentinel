// Package emes defines the EMES-V1 wire event taxonomy as a client-agnostic
// package, separate from the Geth-specific collection mechanism in
// package tracer. This split (wire format vs. collector) was adopted from
// a later design pass and is a genuine improvement: it lets a Reth or Besu
// adapter emit the same emes.Event types without importing anything
// Geth-specific.
package emes

import (
	"encoding/hex"
	"encoding/json"
	"errors"
	"math/big"
)

// --- Hex-marshaling byte types -------------------------------------------
//
// Every EMES-V1 event field that's an address, hash, or 256-bit value uses
// one of these instead of a raw [20]byte/[32]byte, so that
// json.MarshalIndent(fixture) produces readable "0x..." strings instead of
// arrays of small integers (Go's default encoding for fixed-size byte
// arrays). This matches the hex-string convention already used everywhere
// else in this repo (semantic_contract.md, validation_vectors/*.json).

type Address [20]byte

func (a Address) MarshalJSON() ([]byte, error) {
	return json.Marshal("0x" + hex.EncodeToString(a[:]))
}

func (a *Address) UnmarshalJSON(b []byte) error {
	var s string
	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}
	return decodeFixedHex(s, a[:])
}

type Hash [32]byte

func (h Hash) MarshalJSON() ([]byte, error) {
	return json.Marshal("0x" + hex.EncodeToString(h[:]))
}

func (h *Hash) UnmarshalJSON(b []byte) error {
	var s string
	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}
	return decodeFixedHex(s, h[:])
}

func decodeFixedHex(s string, out []byte) error {
	if len(s) < 2 || s[:2] != "0x" {
		return errors.New("emes: hex value must be 0x-prefixed")
	}
	decoded, err := hex.DecodeString(s[2:])
	if err != nil {
		return err
	}
	if len(decoded) != len(out) {
		return errors.New("emes: hex value has wrong byte length")
	}
	copy(out, decoded)
	return nil
}

// BigToHash big-endian-encodes v into a Hash, per EMES-V1 §1.1's canonical
// Ethereum execution (big-endian) rule for 256-bit quantities.
func BigToHash(v *big.Int) Hash {
	var out Hash
	if v == nil {
		return out
	}
	v.FillBytes(out[:])
	return out
}

// --- Event / MutableEvent interfaces --------------------------------------

type Event interface {
	SequenceNum() uint64
	Kind() string
}

type MutableEvent interface {
	Event
	SetSequence(seq uint64)
}

// base is embedded (by value) in every concrete event type below so they
// all get SequenceNum/SetSequence for free via promoted pointer-receiver
// methods -- avoids repeating this boilerplate 12 times.
type base struct {
	Sequence uint64 `json:"sequence"`
}

func (b *base) SequenceNum() uint64    { return b.Sequence }
func (b *base) SetSequence(s uint64)   { b.Sequence = s }

// --- Frame classification --------------------------------------------------

type FrameType uint8

const (
	FrameCall FrameType = iota
	FrameCallCode
	FrameDelegateCall
	FrameStaticCall
	FrameCreate
	FrameCreate2
	FramePrecompile
)

// --- EMES-V1-RC3 structural + mutation events -----------------------------
// Field names (FrameID/ParentFrameID, not Fid/ParentFid) are kept identical
// to what's already committed in tracer/kaysentinel_tracer.go and
// docs/emes_profile.md -- this package is that same taxonomy moved to its
// own package, not a re-design.

type BlockStartEvent struct {
	base
	EMESVersion string
	Number      uint64
	Timestamp   uint64
}

type TransactionStartEvent struct {
	base
	TxIndex  uint64
	Hash     Hash
	From     Address
	To       *Address // nil indicates explicit contract deployment
	Value    Hash
	GasLimit uint64
	GasPrice Hash
	Nonce    uint64
}

type FrameEnterEvent struct {
	base
	FrameID       uint64
	ParentFrameID uint64
	Depth         uint64
	Type          FrameType
	From          Address
	To            Address
}

type FrameExitEvent struct {
	base
	FrameID  uint64
	Reverted bool
}

type TransactionEndEvent struct {
	base
	TxIndex  uint64
	Reverted bool
}

type BlockCommitEvent struct {
	base
	StateRoot Hash
}

type BalanceMutationEvent struct {
	base
	FrameID uint64
	Address Address
	Before  Hash
	After   Hash
}

type NonceMutationEvent struct {
	base
	FrameID uint64
	Address Address
	Before  uint64
	After   uint64
}

type CodeMutationEvent struct {
	base
	FrameID uint64
	Address Address
	Before  Hash // code hash, before
	After   Hash // code hash, after
}

type StorageMutationEvent struct {
	base
	FrameID uint64
	Address Address
	Slot    Hash
	Before  Hash
	After   Hash
}

type AccountCreatedEvent struct {
	base
	FrameID uint64
	Address Address
	Creator Address
}

type SelfDestructEvent struct {
	base
	FrameID     uint64
	Address     Address
	Beneficiary Address
}

// --- Environment description ----------------------------------------------

// EnvironmentDescriptor lets the tracer apply fork-aware rules (e.g. is a
// given address a precompile under the active fork) and lets the harness
// stamp accurate provenance metadata into fixture files, without either one
// hard-coding a specific client or fork.
type EnvironmentDescriptor interface {
	ChainID() *big.Int
	ForkName() string
	ClientName() string
	ClientVersion() string
	IsPrecompile(addr Address) bool
}

// --- Structured tracer diagnostics -----------------------------------------
//
// Adopted from a later design pass: internal tracer anomalies (an EXIT with
// no matching ENTER, a sequence overflow, etc.) get recorded as structured,
// machine-readable errors instead of being silently dropped or turned into
// unstructured log strings.

type InternalErrorCode uint16

const (
	ErrEmptyFrameStack InternalErrorCode = iota
	ErrDuplicateExit
	ErrUnexpectedExit
	ErrSequenceOverflow
)

type InternalTracerError struct {
	Code     InternalErrorCode `json:"code"`
	FrameID  uint64            `json:"frame_id"`
	Sequence uint64            `json:"sequence"`
	Message  string            `json:"message"`
}

func (e *BlockStartEvent) Kind() string       { return "BlockStart" }
func (e *TransactionStartEvent) Kind() string { return "TransactionStart" }
func (e *FrameEnterEvent) Kind() string       { return "FrameEnter" }
func (e *FrameExitEvent) Kind() string        { return "FrameExit" }
func (e *TransactionEndEvent) Kind() string   { return "TransactionEnd" }
func (e *BlockCommitEvent) Kind() string      { return "BlockCommit" }
func (e *BalanceMutationEvent) Kind() string  { return "BalanceMutation" }
func (e *NonceMutationEvent) Kind() string    { return "NonceMutation" }
func (e *CodeMutationEvent) Kind() string     { return "CodeMutation" }
func (e *StorageMutationEvent) Kind() string  { return "StorageMutation" }
func (e *AccountCreatedEvent) Kind() string   { return "AccountCreated" }
func (e *SelfDestructEvent) Kind() string     { return "SelfDestruct" }

// --- Fixture provenance + envelope ------------------------------------------

type FixtureMetadata struct {
	Profile          string `json:"profile"`
	ChainID          int64  `json:"chain_id"`
	Network          string `json:"network"`
	Fork             string `json:"fork"`
	Client           string `json:"client"`
	ClientVersion    string `json:"client_version"`
	BlockNumber      uint64 `json:"block_number"`
	TransactionHash  Hash   `json:"transaction_hash"`
	GeneratorVersion string `json:"generator_version"`
}

type FixtureEnvelope struct {
	Metadata FixtureMetadata       `json:"metadata"`
	Events   []Event               `json:"events"`
	Errors   []InternalTracerError `json:"errors,omitempty"`
}

// MarshalJSON tags each event with its Kind() so a reader can tell a
// BlockStartEvent from a FrameEnterEvent from the JSON alone -- plain
// encoding/json on a []Event slice would marshal each element's fields fine,
// but nothing in that output identifies *which* event type produced them.
// (This package intentionally does not implement UnmarshalJSON for
// FixtureEnvelope: reconstructing concrete Event types from the "type" tag
// on read-back is left to whatever consumes these fixtures, since that
// consumer doesn't exist yet -- see docs/emes_profile.md.)
func (f FixtureEnvelope) MarshalJSON() ([]byte, error) {
	type taggedEvent struct {
		Type string          `json:"type"`
		Data json.RawMessage `json:"data"`
	}
	tagged := make([]taggedEvent, 0, len(f.Events))
	for _, e := range f.Events {
		raw, err := json.Marshal(e)
		if err != nil {
			return nil, err
		}
		tagged = append(tagged, taggedEvent{Type: e.Kind(), Data: raw})
	}
	out := struct {
		Metadata FixtureMetadata        `json:"metadata"`
		Events   []taggedEvent          `json:"events"`
		Errors   []InternalTracerError  `json:"errors,omitempty"`
	}{
		Metadata: f.Metadata,
		Events:   tagged,
		Errors:   f.Errors,
	}
	return json.Marshal(out)
}
