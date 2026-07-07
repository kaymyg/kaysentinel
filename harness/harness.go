// Package harness implements the fixture-producing side of the conformance
// architecture: run Gate 1 over a tracer's collected stream, then write a
// self-describing FixtureEnvelope to a path stamped with the
// EnvironmentDescriptor's provenance.
package harness

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"kaysentinel/emes"
	"kaysentinel/validation"
)

type ConformanceHarness struct {
	BaseFixtureDir string
	NetworkName    string // e.g. "ethereum-mainnet", "sepolia"
}

func New(baseDir, networkName string) *ConformanceHarness {
	return &ConformanceHarness{BaseFixtureDir: baseDir, NetworkName: networkName}
}

// EventsProducer is anything that can hand back a collected EMES-V1 stream
// and its structured error log -- deliberately a small interface (not a
// concrete *tracer.Tracer dependency) so this package doesn't need to
// import the Geth-specific tracer package, keeping the client-agnostic
// promise from docs/emes_profile.md intact at the package-dependency level
// too.
type EventsProducer interface {
	Events() []emes.Event
	ErrorLog() []emes.InternalTracerError
}

// ProcessAndSerialize runs Gate 1 over the producer's collected stream and,
// if it passes, writes a FixtureEnvelope to
// <BaseFixtureDir>/<network>/<fork>/<client>-<version>/<scenarioID>.json
func (h *ConformanceHarness) ProcessAndSerialize(
	scenarioID string,
	blockNum uint64,
	txHash emes.Hash,
	producer EventsProducer,
	env emes.EnvironmentDescriptor,
) error {
	stream := producer.Events()

	if err := validation.VerifyGate1Invariants(stream); err != nil {
		return fmt.Errorf("gate 1 structural verification failed for %s: %w", scenarioID, err)
	}

	envelope := emes.FixtureEnvelope{
		Metadata: emes.FixtureMetadata{
			Profile:          "EMES-V1",
			ChainID:          env.ChainID().Int64(),
			Network:          h.NetworkName,
			Fork:             env.ForkName(),
			Client:           env.ClientName(),
			ClientVersion:    env.ClientVersion(),
			BlockNumber:      blockNum,
			TransactionHash:  txHash,
			GeneratorVersion: "0.1.0",
		},
		Events: stream,
		Errors: producer.ErrorLog(),
	}

	targetDir := filepath.Join(
		h.BaseFixtureDir,
		h.NetworkName,
		env.ForkName(),
		fmt.Sprintf("%s-%s", env.ClientName(), env.ClientVersion()),
	)
	if err := os.MkdirAll(targetDir, 0o755); err != nil {
		return fmt.Errorf("failed to create fixture directory: %w", err)
	}

	data, err := json.MarshalIndent(envelope, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to encode fixture envelope: %w", err)
	}

	fullPath := filepath.Join(targetDir, scenarioID+".json")
	if err := os.WriteFile(fullPath, data, 0o644); err != nil {
		return fmt.Errorf("failed to write fixture: %w", err)
	}

	fmt.Printf("[conformance] wrote verified fixture: %s\n", fullPath)
	return nil
}
