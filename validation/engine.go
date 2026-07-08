// Package validation (engine) provides field-level differential comparison
// between a "legacy" SemanticEvent stream and one derived from
// raw/transcript, for use during a migration/verification period where
// both tracers run side by side.
package validation

import (
	"bytes"
	"fmt"
)

type DifferentialValidator struct{}

func NewDifferentialValidator() *DifferentialValidator { return &DifferentialValidator{} }

func (v *DifferentialValidator) formatMismatch(field string, seq uint64, op string, depth int, exp, act any) string {
	buf := new(bytes.Buffer)
	fmt.Fprintf(buf, "\n[CRITICAL SEMANTIC DRIFT] Mismatch Isolated Across Stream Pipeline\n")
	fmt.Fprintf(buf, "Sequence : %d\n", seq)
	fmt.Fprintf(buf, "Event     : %s\n", op)
	fmt.Fprintf(buf, "Depth     : %d\n\n", depth)
	fmt.Fprintf(buf, "%-12s %-32s %-32s\n", "Field", "Expected (Legacy)", "Actual (Transcript)")
	fmt.Fprintf(buf, "------------------------------------------------------------------------\n")
	fmt.Fprintf(buf, "%-12s %-32v %-32v\n", field, exp, act)
	return buf.String()
}

// CompareSemanticFields diffs a legacy-tracer-derived event l against a
// transcript-derived event t, returning nil if they match on every field
// SemanticEvent carries.
func (v *DifferentialValidator) CompareSemanticFields(l, t SemanticEvent) error {
	if l.Sequence != t.Sequence {
		return fmt.Errorf("sequence split: legacy=%d, transcript=%d", l.Sequence, t.Sequence)
	}
	if l.Opcode != t.Opcode {
		return fmt.Errorf("%s", v.formatMismatch("Opcode", l.Sequence, l.Opcode, l.Depth, l.Opcode, t.Opcode))
	}
	if l.Depth != t.Depth {
		return fmt.Errorf("%s", v.formatMismatch("Depth", l.Sequence, l.Opcode, l.Depth, l.Depth, t.Depth))
	}
	if l.From != t.From {
		return fmt.Errorf("%s", v.formatMismatch("FromAddress", l.Sequence, l.Opcode, l.Depth, l.From.Hex(), t.From.Hex()))
	}
	if l.To != t.To {
		return fmt.Errorf("%s", v.formatMismatch("ToAddress", l.Sequence, l.Opcode, l.Depth, l.To.Hex(), t.To.Hex()))
	}
	if !l.Value.Eq(&t.Value) {
		return fmt.Errorf("%s", v.formatMismatch("AssetValue", l.Sequence, l.Opcode, l.Depth, l.Value.Hex(), t.Value.Hex()))
	}
	if l.GasUsed != t.GasUsed {
		return fmt.Errorf("%s", v.formatMismatch("GasUsed", l.Sequence, l.Opcode, l.Depth, l.GasUsed, t.GasUsed))
	}
	if l.Reverted != t.Reverted {
		return fmt.Errorf("%s", v.formatMismatch("Reverted", l.Sequence, l.Opcode, l.Depth, l.Reverted, t.Reverted))
	}
	if l.ErrType != t.ErrType {
		return fmt.Errorf("%s", v.formatMismatch("ErrType", l.Sequence, l.Opcode, l.Depth, l.ErrType, t.ErrType))
	}
	return nil
}
