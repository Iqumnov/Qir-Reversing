# Quantum Deobfuscation Passes

**QIR Reversing** implements multi-pass static bytecode optimization passes designed to strip artificial obfuscation injected into quantum intermediate representations while strictly preserving unitary equivalence modulo global phase on observable qubits:

$$U_{\text{clean}} \equiv e^{i\phi} U_{\text{obfuscated}}$$

---

## 1. Dead Qubit Telemetry Elimination (`DeadQubitPruning`)

* **Pattern Signature**: `DEAD_QUBIT_INJECTION`
* **Severity**: `HIGH`
* **Mechanism**:
  Obfuscators inject auxiliary qubits ($q_{\text{dead}}$) with dummy gates or unmeasured ancillae to inflate circuit volume.
* **Algorithm**:
  1. Construct measurement target set: $\mathcal{M} = \{ q \mid \exists \text{Mz}(q, r) \text{ or custom result readout} \}$.
  2. Compute backward causal cone: any multi-qubit gate ($CX$, $CZ$, $\text{SWAP}$, $CCX$) entangled with a qubit in $\mathcal{M}$ is pulled into $\mathcal{M}$.
  3. Prune all gates acting exclusively on $Q \setminus \mathcal{M}$.

---

## 2. Involutory Self-Inverse Gate Pair Cancellation (`InvolutionCancellation`)

* **Pattern Signature**: `IDENTITY_INVOLUTION_PAIR`
* **Severity**: `HIGH`
* **Mechanism**:
  Unitary operators that satisfy $G^2 = I$:
  $$H^2 = I, \quad X^2 = I, \quad Y^2 = I, \quad Z^2 = I, \quad CX^2 = I, \quad CZ^2 = I, \quad \text{SWAP}^2 = I, \quad CCX^2 = I$$
* **Algorithm**:
  Lookahead peephole scanner searches down the wire for matching involutory gates. If all intervening gates commute with $G$, the pair cancels to Identity.

---

## 3. Discrete Clifford+T Phase Group Reduction (`DiscretePhaseConsolidation`)

* **Pattern Signatures**: `DISCRETE_PHASE_CANCELLATION` (`HIGH`) / `DISCRETE_PHASE_CONSOLIDATION` (`MEDIUM`)
* **Mechanism**:
  Elements $\{T, S, Z, S^\dagger, T^\dagger\}$ form the cyclic subgroup $\mathbb{Z}_8$ under phase addition mod $8$: inverse pairs ($S \cdot S^\dagger$, $T \cdot T^\dagger$) cancel, while non-trivial sums fold (e.g., $T \cdot T \to S$, $S \cdot S \to Z$, $S^\dagger \cdot Z \to S$).

---

## 4. Continuous Rotation Consolidation (`RotationConsolidation`)

* **Pattern Signatures**: `TRIVIAL_IDENTITY_ROTATION` (`HIGH`) / `INVERSE_ROTATION_CANCELLATION` (`HIGH`) / `CONTINUOUS_ROTATION_CONSOLIDATION` (`MEDIUM`)
* **Mechanism**:
  Single-qubit rotations around the same axis ($R_x$, $R_y$, $R_z$) satisfy:
  $$R_\sigma(\theta_1) \cdot R_\sigma(\theta_2) = R_\sigma(\theta_1 + \theta_2)$$
  Standalone or summed angles $\equiv 0 \pmod{2\pi}$ collapse to Identity up to unobservable global phase ($\pm 1$).

