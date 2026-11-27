# Unitary Matrix Simulation & Hilbert Space Verification

## 1. Hilbert Space Formulation

For an $N$-qubit quantum register, the state space is the tensor product Hilbert space:

$$\mathcal{H} = (\mathbb{C}^2)^{\otimes N} \cong \mathbb{C}^{2^N}$$

A circuit consisting of a sequential gate stream $G_1, G_2, \dots, G_m$ transforms the computational basis states under the total unitary operator:

$$U_{tot} = G_m \cdots G_2 G_1$$

## 2. Gate Matrix Definitions

### 2.1 Single-Qubit Operators
$$\sigma_x = \begin{pmatrix} 0 & 1 \\ 1 & 0 \end{pmatrix}, \quad \sigma_y = \begin{pmatrix} 0 & -i \\ i & 0 \end{pmatrix}, \quad \sigma_z = \begin{pmatrix} 1 & 0 \\ 0 & -1 \end{pmatrix}$$

$$H = \frac{1}{\sqrt{2}} \begin{pmatrix} 1 & 1 \\ 1 & -1 \end{pmatrix}, \quad S = \begin{pmatrix} 1 & 0 \\ 0 & i \end{pmatrix}, \quad T = \begin{pmatrix} 1 & 0 \\ 0 & e^{i\pi/4} \end{pmatrix}$$

$$R_x(\theta) = \begin{pmatrix} \cos(\theta/2) & -i \sin(\theta/2) \\ -i \sin(\theta/2) & \cos(\theta/2) \end{pmatrix}$$

$$R_z(\theta) = \begin{pmatrix} e^{-i\theta/2} & 0 \\ 0 & e^{i\theta/2} \end{pmatrix}$$

## 3. Equivalence Verification Metrics

### 3.1 Phase-Aligned Frobenius Distance
$$\min_{\phi \in [0, 2\pi)} \|U_1 - e^{i\phi} U_2\|_F = \min_{\phi} \sqrt{\sum_{i,j} |(U_1)_{ij} - e^{i\phi}(U_2)_{ij}|^2}$$

A phase-aligned Frobenius distance $< 10^{-6}\sqrt{d}$ (together with unitarity drift $\|U \cdot U^\dagger - I\|_F < \text{unitarity\_tolerance}(N)$) establishes numerical equivalence modulo global phase between original and deobfuscated circuits.

### 3.2 Quantum Process Fidelity
In physical quantum systems, global phase differences ($U_1 = e^{i\phi} U_2$) have no observable consequence. The phase-invariant quantum gate fidelity is:

$$F(U_1, U_2) = \frac{1}{d^2} \left| \text{Tr}(U_1^\dagger U_2) \right|^2$$

Where $d = 2^N$. $F(U_1, U_2) = 1.0$ proves that both quantum circuits implement the identical physical transformation.

## 4. Scaling Beyond 12 Qubits: Canonical QMDD Alternating Verification (`qir_core::dd`)

Beyond 12 qubits natively (or 10 qubits in `wasm32`), dense $2^N \times 2^N$ matrices exceed practical memory bounds. `qir-core::dd` switches automatically to Canonical Quantum Multiple-Valued Decision Diagrams (QMDD) up to **64 qubits**:

- **Canonical Normalization**: Uses hybrid max-magnitude first-edge canonical pivoting with quantized unique-table lookup and Frobenius-norm-consistent traces.
- **Alternating Strategy ($G \to \mathbb{I} \leftarrow H^\dagger$)**: Interleaves left-multiplication by $G_i$ and right-multiplication by $H_j^\dagger$ with identity-seeking lookahead when $D \neq \mathbb{I}$, collapsing front/rear obfuscations immediately toward identity.
- **Scale-Consistent Reversibility Bound**: Uses $\text{unitarity\_tolerance}(N) = 10^{-9} \cdot 2^{(N-12)/2}$ for $N > 12$ (matching dense $10^{-9}$ for $N \le 12$).

