# Quantum Deobfuscation Passes

**QIR Reversing** implements multi-pass static bytecode optimization passes designed to strip artificial obfuscation injected into quantum intermediate representations while strictly preserving unitary equivalence:

$$U_{\text{clean}} = U_{\text{obfuscated}}$$

---

## 1. Dead Qubit Telemetry Elimination (`DeadQubitPruning`)

* **Pattern Signature**: `DEAD_QUBIT_INJECTION`
* **Severity**: `HIGH`
* **Mechanism**:
  Obfuscators inject auxiliary qubits ($q_{\text{dead}}$) with dummy single-qubit gates ($H$, $X$, $R_z$) or unmeasured ancillae to inflate circuit volume.
* **Algorithm**:
  1. Construct measurement target set: $\mathcal{M} = \{ q \mid \exists \text{Mz}(q, r) \}$.
  2. Compute backward causal cone: any multi-qubit gate ($CX$, $CZ$, $CCX$) entangled with a qubit in $\mathcal{M}$ is pulled into $\mathcal{M}$.
  3. Prune all gates acting exclusively on $Q \setminus \mathcal{M}$.

---

## 2. Involutory Self-Inverse Gate Pair Cancellation (`InvolutionCancellation`)

* **Pattern Signature**: `IDENTITY_INVOLUTION_PAIR`
* **Severity**: `HIGH`
* **Mechanism**:
  Unitary operators that satisfy $G^2 = I$:
  $$H \cdot H = I, \quad X \cdot X = I, \quad Y \cdot Y = I, \quad Z \cdot Z = I$$
  $$CX(c, t) \cdot CX(c, t) = I, \quad CZ(c, t) \cdot CZ(c, t) = I$$
* **Algorithm**:
  Lookahead peephole scanner searches down the wire for consecutive identical involutory gates. If intervening gates commute with $G$, the pair is merged and reduced to the Identity operator.

---

## 3. Continuous Rotation Consolidation (`RotationConsolidation`)

* **Pattern Signature**: `INVERSE_ROTATION_CANCELLATION` / `CONTINUOUS_ROTATION_CONSOLIDATION`
* **Severity**: `HIGH` / `MEDIUM`
* **Mechanism**:
  Phase rotations around the same axis form an abelian subgroup isomorphic to $U(1)$:
  $$R_z(\theta_1) \cdot R_z(\theta_2) = R_z(\theta_1 + \theta_2)$$
  If $(\theta_1 + \theta_2) \equiv 0 \pmod{2\pi}$, the rotation collapses to Identity ($I$) up to an unobservable global phase ($e^{i \cdot 0} = 1$ or $e^{i\pi} = -1$).

