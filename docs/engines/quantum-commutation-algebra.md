# Quantum Commutation Algebra & Deobfuscation Theory

## 1. Commutation Rules

In quantum circuit optimization, two gates $A$ and $B$ can be reordered in time without altering circuit behavior if and only if their commutator vanishes:

$$[A, B] = A B - B A = 0$$

If $[A, B] = 0$, gate $B$ can slide past gate $A$ in a peephole optimization window to cancel against an adjacent inverse gate $B^\dagger$.

### 1.1 Spatial Disjointness
If gates $A$ and $B$ act on disjoint sets of qubits $\text{supp}(A) \cap \text{supp}(B) = \emptyset$, their tensor products commute unconditionally:

$$[A \otimes I, I \otimes B] = 0$$

### 1.2 Pauli $Z$-Family Commutation
Any pair of diagonal single-qubit gates commute:

$$[Z, S] = [Z, T] = [S, T] = [R_z(\alpha), R_z(\beta)] = 0$$

### 1.3 Pauli $X$-Family Commutation
Similarly, single-qubit bit-flip operations commute along the $X$-basis:

$$[X, R_x(\alpha)] = [R_x(\alpha), R_x(\beta)] = 0$$

### 1.4 Controlled-NOT Commutation
For $CX(c, t)$ with control qubit $c$ and target qubit $t$:
- $CX(c, t)$ commutes with $Z$-family gates on control qubit $c$:
  $$[CX(c, t), Z_c] = 0, \quad [CX(c, t), R_z(\theta)_c] = 0$$
- $CX(c, t)$ commutes with $X$-family gates on target qubit $t$:
  $$[CX(c, t), X_t] = 0, \quad [CX(c, t), R_x(\theta)_t] = 0$$

### 1.5 Controlled-$Z$ Commutation
Because $CZ(c, t) = \text{diag}(1, 1, 1, -1)$ is purely diagonal:
- $CZ(c, t)$ is symmetric: $CZ(c, t) = CZ(t, c)$.
- $CZ$ commutes with any $Z$-family gate on either wire $c$ or $t$:
  $$[CZ(c, t), Z_c] = [CZ(c, t), Z_t] = 0$$
- Any two $CZ$ gates commute:
  $$[CZ(c_1, t_1), CZ(c_2, t_2)] = 0$$

## 2. Involutory Pair Reduction
A quantum gate $G$ is involutory if $G^2 = I$. The following gates are self-inverse:
$$H^2 = I, \quad X^2 = I, \quad Y^2 = I, \quad Z^2 = I$$
$$CX(c, t)^2 = I, \quad CZ(c, t)^2 = I, \quad \text{SWAP}^2 = I, \quad CCX(c_1, c_2, t)^2 = I$$

When two identical involutory gates are separated only by gates that commute with them, the intervening gates do not block commutation, allowing the pair to collapse to the Identity operator $I$.

