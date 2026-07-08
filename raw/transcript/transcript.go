package transcript

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"fmt"
	"math/big"
	"reflect"

	"github.com/ethereum/go-ethereum/common"
	"github.com/holiman/uint256"
)

const (
	CanonicalVersion uint16 = 1
	StreamMagic             = "KTS1"
)

type EventKind uint16

const (
	EvTxStart EventKind = iota
	EvTxEnd
	EvEnter
	EvExit
	EvFault
	EvBalanceChange
)

func (k EventKind) String() string {
	switch k {
	case EvTxStart:
		return "TX_START"
	case EvTxEnd:
		return "TX_END"
	case EvEnter:
		return "ENTER"
	case EvExit:
		return "EXIT"
	case EvFault:
		return "FAULT"
	case EvBalanceChange:
		return "BALANCE_CHANGE"
	default:
		return fmt.Sprintf("UNKNOWN_EVENT_KIND(%d)", k)
	}
}

type ProjectedPayload interface {
	isProjectedPayload()
}

type ErrorSnapshot struct {
	Type    string
	Message string
}

type ProjectedTxStart struct {
	GasLimit uint64
}

func (ProjectedTxStart) isProjectedPayload() {}

type ProjectedTxEnd struct {
	GasUsed uint64
	HasErr  bool
	Err     ErrorSnapshot
}

func (ProjectedTxEnd) isProjectedPayload() {}

type ProjectedEnter struct {
	Depth    int
	CallType byte
	From     common.Address
	To       common.Address
	Gas      uint64
	HasValue bool
	Value    uint256.Int
}

func (ProjectedEnter) isProjectedPayload() {}

type ProjectedExit struct {
	Depth    int
	Output   []byte
	GasUsed  uint64
	HasErr   bool
	Err      ErrorSnapshot
	Reverted bool
}

func (ProjectedExit) isProjectedPayload() {}

type ProjectedFault struct {
	Depth  int
	PC     uint64
	Op     byte
	Gas    uint64
	Cost   uint64
	HasErr bool
	Err    ErrorSnapshot
}

func (ProjectedFault) isProjectedPayload() {}

type ProjectedBalanceChange struct {
	Address  common.Address
	HasOld   bool
	OldSign  int
	OldBytes []byte
	HasNew   bool
	NewSign  int
	NewBytes []byte
	Reason   uint32
}

func (ProjectedBalanceChange) isProjectedPayload() {}

type ProjectedEvent struct {
	Seq  uint64
	Kind EventKind
	Data ProjectedPayload
}

type tracePayload interface {
	isTracePayload()
}

type txStartData struct{ GasLimit uint64 }

func (txStartData) isTracePayload() {}

type txEndData struct {
	GasUsed uint64
	Err     *ErrorSnapshot
}

func (txEndData) isTracePayload() {}

type enterData struct {
	Depth    int
	CallType byte
	From     common.Address
	To       common.Address
	Gas      uint64
	Value    *uint256.Int
}

func (enterData) isTracePayload() {}

type exitData struct {
	Depth    int
	Output   []byte
	GasUsed  uint64
	Err      *ErrorSnapshot
	Reverted bool
}

func (exitData) isTracePayload() {}

type faultData struct {
	Depth int
	PC    uint64
	Op    byte
	Gas   uint64
	Cost  uint64
	Err   *ErrorSnapshot
}

func (faultData) isTracePayload() {}

type balanceChangeData struct {
	Address common.Address
	Old     *big.Int
	New     *big.Int
	Reason  uint32
}

func (balanceChangeData) isTracePayload() {}

type nativeEvent struct {
	kind EventKind
	data tracePayload
}

type ExecutionTranscript struct {
	events []nativeEvent
}

func NewExecutionTranscript() *ExecutionTranscript {
	return &ExecutionTranscript{events: make([]nativeEvent, 0, 128)}
}

func (t *ExecutionTranscript) Len() int { return len(t.events) }

func makeSnapshot(err error) *ErrorSnapshot {
	if err == nil {
		return nil
	}
	return &ErrorSnapshot{
		Type:    reflect.TypeOf(err).String(),
		Message: err.Error(),
	}
}

func (t *ExecutionTranscript) AppendTxStart(gasLimit uint64) {
	t.events = append(t.events, nativeEvent{kind: EvTxStart, data: txStartData{GasLimit: gasLimit}})
}

func (t *ExecutionTranscript) AppendTxEnd(gasUsed uint64, err error) {
	t.events = append(t.events, nativeEvent{kind: EvTxEnd, data: txEndData{GasUsed: gasUsed, Err: makeSnapshot(err)}})
}

func (t *ExecutionTranscript) AppendEnter(depth int, callType byte, from, to common.Address, gas uint64, val *uint256.Int) {
	var clonedVal *uint256.Int
	if val != nil {
		clonedVal = val.Clone()
	}
	t.events = append(t.events, nativeEvent{kind: EvEnter, data: enterData{
		Depth: depth, CallType: callType, From: from, To: to, Gas: gas, Value: clonedVal,
	}})
}

func (t *ExecutionTranscript) AppendExit(depth int, output []byte, gasUsed uint64, err error, reverted bool) {
	var clonedOut []byte
	if output != nil {
		clonedOut = make([]byte, len(output))
		copy(clonedOut, output)
	}
	t.events = append(t.events, nativeEvent{kind: EvExit, data: exitData{
		Depth: depth, Output: clonedOut, GasUsed: gasUsed, Err: makeSnapshot(err), Reverted: reverted,
	}})
}

func (t *ExecutionTranscript) AppendFault(depth int, pc uint64, op byte, gas, cost uint64, err error) {
	t.events = append(t.events, nativeEvent{kind: EvFault, data: faultData{
		Depth: depth, PC: pc, Op: op, Gas: gas, Cost: cost, Err: makeSnapshot(err),
	}})
}

func (t *ExecutionTranscript) AppendBalanceChange(addr common.Address, oldBal, newBal *big.Int, reason uint32) {
	var cpOld, cpNew *big.Int
	if oldBal != nil {
		cpOld = new(big.Int).Set(oldBal)
	}
	if newBal != nil {
		cpNew = new(big.Int).Set(newBal)
	}
	t.events = append(t.events, nativeEvent{kind: EvBalanceChange, data: balanceChangeData{
		Address: addr, Old: cpOld, New: cpNew, Reason: reason,
	}})
}

func (t *ExecutionTranscript) StreamProjection(receiver func(ProjectedEvent) error) error {
	for idx, ev := range t.events {
		var projPayload ProjectedPayload

		switch d := ev.data.(type) {
		case txStartData:
			projPayload = ProjectedTxStart{GasLimit: d.GasLimit}
		case txEndData:
			pEnd := ProjectedTxEnd{GasUsed: d.GasUsed}
			if d.Err != nil {
				pEnd.HasErr = true
				pEnd.Err = *d.Err
			}
			projPayload = pEnd
		case enterData:
			pEnter := ProjectedEnter{
				Depth: d.Depth, CallType: d.CallType, From: d.From, To: d.To, Gas: d.Gas,
			}
			if d.Value != nil {
				pEnter.HasValue = true
				pEnter.Value = *d.Value
			}
			projPayload = pEnter
		case exitData:
			pExit := ProjectedExit{
				Depth: d.Depth, GasUsed: d.GasUsed, Reverted: d.Reverted,
			}
			if d.Output != nil {
				pExit.Output = make([]byte, len(d.Output))
				copy(pExit.Output, d.Output)
			}
			if d.Err != nil {
				pExit.HasErr = true
				pExit.Err = *d.Err
			}
			projPayload = pExit
		case faultData:
			pFault := ProjectedFault{
				Depth: d.Depth, PC: d.PC, Op: d.Op, Gas: d.Gas, Cost: d.Cost,
			}
			if d.Err != nil {
				pFault.HasErr = true
				pFault.Err = *d.Err
			}
			projPayload = pFault
		case balanceChangeData:
			pBal := ProjectedBalanceChange{Address: d.Address, Reason: d.Reason}
			if d.Old != nil {
				pBal.HasOld = true
				pBal.OldSign = d.Old.Sign()
				pBal.OldBytes = d.Old.Bytes()
			}
			if d.New != nil {
				pBal.HasNew = true
				pBal.NewSign = d.New.Sign()
				pBal.NewBytes = d.New.Bytes()
			}
			projPayload = pBal
		default:
			return fmt.Errorf("fatal: unmapped internal storage payload variant encountered: %T", ev.data)
		}

		projectedEvent := ProjectedEvent{
			Seq:  uint64(idx),
			Kind: ev.kind,
			Data: projPayload,
		}

		if err := receiver(projectedEvent); err != nil {
			return err
		}
	}
	return nil
}

func (t *ExecutionTranscript) ComputeDeterministicBinaryHashV1() ([32]byte, error) {
	buf := new(bytes.Buffer)

	if _, err := buf.WriteString(StreamMagic); err != nil {
		return [32]byte{}, err
	}
	if err := binary.Write(buf, binary.BigEndian, CanonicalVersion); err != nil {
		return [32]byte{}, err
	}
	if err := binary.Write(buf, binary.BigEndian, uint16(0)); err != nil {
		return [32]byte{}, err
	}

	if err := binary.Write(buf, binary.BigEndian, uint64(len(t.events))); err != nil {
		return [32]byte{}, err
	}

	for _, ev := range t.events {
		if err := binary.Write(buf, binary.BigEndian, uint16(ev.kind)); err != nil {
			return [32]byte{}, err
		}
		if err := encodePayloadV1(buf, ev.data); err != nil {
			return [32]byte{}, err
		}
	}

	return sha256.Sum256(buf.Bytes()), nil
}

func encodePayloadV1(buf *bytes.Buffer, payload tracePayload) error {
	switch d := payload.(type) {
	case txStartData:
		if err := binary.Write(buf, binary.BigEndian, d.GasLimit); err != nil {
			return err
		}
	case txEndData:
		if err := binary.Write(buf, binary.BigEndian, d.GasUsed); err != nil {
			return err
		}
		if err := writeErrorSnapshotV1(buf, d.Err); err != nil {
			return err
		}
	case enterData:
		if err := binary.Write(buf, binary.BigEndian, int64(d.Depth)); err != nil {
			return err
		}
		if err := buf.WriteByte(d.CallType); err != nil {
			return err
		}
		if _, err := buf.Write(d.From.Bytes()); err != nil {
			return err
		}
		if _, err := buf.Write(d.To.Bytes()); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.Gas); err != nil {
			return err
		}
		if err := writeUint256V1(buf, d.Value); err != nil {
			return err
		}
	case exitData:
		if err := binary.Write(buf, binary.BigEndian, int64(d.Depth)); err != nil {
			return err
		}
		if err := writeSliceV1(buf, d.Output); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.GasUsed); err != nil {
			return err
		}
		if err := writeErrorSnapshotV1(buf, d.Err); err != nil {
			return err
		}
		if err := writeBoolV1(buf, d.Reverted); err != nil {
			return err
		}
	case faultData:
		if err := binary.Write(buf, binary.BigEndian, int64(d.Depth)); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.PC); err != nil {
			return err
		}
		if err := buf.WriteByte(d.Op); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.Gas); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.Cost); err != nil {
			return err
		}
		if err := writeErrorSnapshotV1(buf, d.Err); err != nil {
			return err
		}
	case balanceChangeData:
		if _, err := buf.Write(d.Address.Bytes()); err != nil {
			return err
		}
		if err := writeBigIntV1(buf, d.Old); err != nil {
			return err
		}
		if err := writeBigIntV1(buf, d.New); err != nil {
			return err
		}
		if err := binary.Write(buf, binary.BigEndian, d.Reason); err != nil {
			return err
		}
	default:
		return fmt.Errorf("canonical encoder missing structural variant case handler: %T", payload)
	}
	return nil
}

func writeBoolV1(buf *bytes.Buffer, b bool) error {
	if b {
		return buf.WriteByte(0x01)
	}
	return buf.WriteByte(0x00)
}

func writeSliceV1(buf *bytes.Buffer, b []byte) error {
	if b == nil {
		return binary.Write(buf, binary.BigEndian, int32(-1))
	}
	if err := binary.Write(buf, binary.BigEndian, int32(len(b))); err != nil {
		return err
	}
	_, err := buf.Write(b)
	return err
}

func writeUint256V1(buf *bytes.Buffer, v *uint256.Int) error {
	if v == nil {
		return buf.WriteByte(0x00)
	}
	if err := buf.WriteByte(0x01); err != nil {
		return err
	}
	var backing [32]byte
	v.WriteToSlice(backing[:])
	_, err := buf.Write(backing[:])
	return err
}

func writeBigIntV1(buf *bytes.Buffer, v *big.Int) error {
	if v == nil {
		return buf.WriteByte(0x00)
	}
	sign := v.Sign()
	if sign == 0 {
		return buf.WriteByte(0x01)
	}
	if sign > 0 {
		if err := buf.WriteByte(0x02); err != nil {
			return err
		}
	} else {
		if err := buf.WriteByte(0x03); err != nil {
			return err
		}
	}
	rawBytes := v.Bytes()
	if err := binary.Write(buf, binary.BigEndian, uint32(len(rawBytes))); err != nil {
		return err
	}
	_, err := buf.Write(rawBytes)
	return err
}

func writeErrorSnapshotV1(buf *bytes.Buffer, snap *ErrorSnapshot) error {
	if snap == nil {
		return buf.WriteByte(0x00)
	}
	if err := buf.WriteByte(0x01); err != nil {
		return err
	}

	if err := binary.Write(buf, binary.BigEndian, uint32(len(snap.Type))); err != nil {
		return err
	}
	if _, err := buf.WriteString(snap.Type); err != nil {
		return err
	}

	if err := binary.Write(buf, binary.BigEndian, uint32(len(snap.Message))); err != nil {
		return err
	}
	_, err := buf.WriteString(snap.Message)
	return err
}
