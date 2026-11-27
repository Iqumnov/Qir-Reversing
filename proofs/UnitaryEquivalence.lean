/-!
-- QIR Reversing Formal Verification: Axiom-Free Unitary Equivalence in Z[ζ₈]

Constructive, kernel-verified Lean 4 proofs (zero `axiom`, zero `sorry`) for:
1. Exact cyclotomic ring Z[ζ₈] (ζ₈ = e^{iπ/4}, ζ₈⁴ = -1, √2 = ζ₈ - ζ₈³) and commutative ring laws.
2. Single-qubit and two-qubit concrete matrix involutions:
   X² = I, Y² = I, Z² = I, (√2·H)² = 2·I, CX² = I₄, CZ² = I₄, SWAP² = I₄.
3. Discrete Clifford+T cyclic group Z₈ (`phaseZ8_add_all`) and universal diagonal commutativity (`diag_comm`).
4. Inductive soundness of arbitrary-distance lookahead cancellation (`lookahead_cancel_word`):
   evaluating `A :: mid ++ [A⁻¹]` equals `evalWord mid` whenever `A` commutes with every gate in `mid`.
5. Multi-qubit commutation, Clifford conjugation, and non-commutation separation theorems (`Mat4`, `Mat8`).
6. Spinor 4π-periodicity of SU(2) rotations Rz(θ):
   Rz(2π) = -I ≠ I; projective equivalence holds mod 2π, while strict equality requires period 4π.

Author: Matvei Igumnov <matvei@iqumnov.com>
Quantum Intermediate Representation Reverse Engineering
-/

namespace QIR.Formal

/-- Exact element `c0 + c1*ζ + c2*ζ² + c3*ζ³` in the cyclotomic ring ℤ[ζ₈],
    where `ζ = e^{iπ/4}` satisfies `ζ⁴ = -1`, `ζ² = i`, and `√2 = ζ - ζ³`. -/
structure Z8 where
  c0 : Int
  c1 : Int
  c2 : Int
  c3 : Int
  deriving DecidableEq, Repr

namespace Z8

def zero : Z8 := ⟨0, 0, 0, 0⟩
def one : Z8 := ⟨1, 0, 0, 0⟩
def negOne : Z8 := ⟨-1, 0, 0, 0⟩
def ofInt (n : Int) : Z8 := ⟨n, 0, 0, 0⟩
def zeta : Z8 := ⟨0, 1, 0, 0⟩
def zeta3Neg : Z8 := ⟨0, 0, 0, -1⟩
def iUnit : Z8 := ⟨0, 0, 1, 0⟩
def iNeg : Z8 := ⟨0, 0, -1, 0⟩
def sqrt2 : Z8 := ⟨0, 1, 0, -1⟩

def add (a b : Z8) : Z8 :=
  ⟨a.c0 + b.c0, a.c1 + b.c1, a.c2 + b.c2, a.c3 + b.c3⟩

def neg (a : Z8) : Z8 :=
  ⟨-a.c0, -a.c1, -a.c2, -a.c3⟩

def mul (a b : Z8) : Z8 :=
  ⟨ a.c0 * b.c0 - a.c1 * b.c3 - a.c2 * b.c2 - a.c3 * b.c1,
    a.c0 * b.c1 + a.c1 * b.c0 - a.c2 * b.c3 - a.c3 * b.c2,
    a.c0 * b.c2 + a.c1 * b.c1 + a.c2 * b.c0 - a.c3 * b.c3,
    a.c0 * b.c3 + a.c1 * b.c2 + a.c2 * b.c1 + a.c3 * b.c0 ⟩

theorem add_comm (a b : Z8) : add a b = add b a := by
  cases a; cases b; simp [add]; constructor <;> omega

theorem add_assoc (a b c : Z8) : add (add a b) c = add a (add b c) := by
  cases a; cases b; cases c
  simp [add]
  refine ⟨by omega, by omega, by omega, by omega⟩

theorem add_swap4 (a b c d : Z8) :
    add (add a b) (add c d) = add (add a c) (add b d) := by
  cases a; cases b; cases c; cases d
  simp only [add, Z8.mk.injEq]
  exact ⟨by omega, by omega, by omega, by omega⟩

theorem zero_add (a : Z8) : add zero a = a := by
  cases a; simp [add, zero]

theorem add_zero (a : Z8) : add a zero = a := by
  cases a; simp [add, zero]

theorem mul_zero (a : Z8) : mul a zero = zero := by
  cases a; simp [mul, zero]

theorem zero_mul (a : Z8) : mul zero a = zero := by
  cases a; simp [mul, zero]

theorem one_mul (a : Z8) : mul one a = a := by
  cases a; simp [mul, one]

theorem mul_comm (a b : Z8) : mul a b = mul b a := by
  cases a; cases b
  simp only [mul, Z8.mk.injEq]
  refine ⟨?_, ?_, ?_, ?_⟩
  · simp only [Int.sub_eq_add_neg, Int.mul_comm]; ac_rfl
  · simp only [Int.sub_eq_add_neg, Int.mul_comm]; ac_rfl
  · simp only [Int.sub_eq_add_neg, Int.mul_comm]; ac_rfl
  · simp only [Int.mul_comm]; ac_rfl

theorem left_distrib (a b c : Z8) : mul a (add b c) = add (mul a b) (mul a c) := by
  cases a; cases b; cases c
  simp only [mul, add, Z8.mk.injEq]
  refine ⟨?_, ?_, ?_, ?_⟩ <;>
    simp only [Int.mul_add, Int.sub_eq_add_neg, Int.neg_add] <;>
    ac_rfl

theorem right_distrib (a b c : Z8) : mul (add a b) c = add (mul a c) (mul b c) := by
  rw [mul_comm, left_distrib, mul_comm c a, mul_comm c b]

theorem mul_assoc (a b c : Z8) : mul (mul a b) c = mul a (mul b c) := by
  cases a; cases b; cases c
  simp only [mul, Z8.mk.injEq]
  refine ⟨?_, ?_, ?_, ?_⟩ <;>
    simp only [Int.mul_add, Int.add_mul,
               Int.mul_neg, Int.neg_mul, Int.neg_neg,
               Int.sub_eq_add_neg, Int.neg_add, Int.mul_assoc] <;>
    ac_rfl

theorem mul_one (a : Z8) : mul a one = a := by
  cases a; simp [mul, one]

theorem sqrt2_squared : mul sqrt2 sqrt2 = ofInt 2 := by
  decide

theorem zeta_fourth : mul (mul zeta zeta) (mul zeta zeta) = negOne := by
  decide

end Z8

/-- Exact 2×2 unitary matrix over ℤ[ζ₈]. -/
structure Mat2 where
  m00 : Z8
  m01 : Z8
  m10 : Z8
  m11 : Z8
  deriving DecidableEq, Repr

namespace Mat2

def mul (A B : Mat2) : Mat2 :=
  ⟨ Z8.add (Z8.mul A.m00 B.m00) (Z8.mul A.m01 B.m10),
    Z8.add (Z8.mul A.m00 B.m01) (Z8.mul A.m01 B.m11),
    Z8.add (Z8.mul A.m10 B.m00) (Z8.mul A.m11 B.m10),
    Z8.add (Z8.mul A.m10 B.m01) (Z8.mul A.m11 B.m11) ⟩

def scale (s : Z8) (A : Mat2) : Mat2 :=
  ⟨ Z8.mul s A.m00, Z8.mul s A.m01, Z8.mul s A.m10, Z8.mul s A.m11 ⟩

def diag (d0 d1 : Z8) : Mat2 := ⟨d0, Z8.zero, Z8.zero, d1⟩

def I2 : Mat2 := diag Z8.one Z8.one
def NegI2 : Mat2 := diag Z8.negOne Z8.negOne
def X2 : Mat2 := ⟨Z8.zero, Z8.one, Z8.one, Z8.zero⟩
def Y2 : Mat2 := ⟨Z8.zero, Z8.iNeg, Z8.iUnit, Z8.zero⟩
def Z2 : Mat2 := diag Z8.one Z8.negOne
def S2 : Mat2 := diag Z8.one Z8.iUnit
def Sdg2 : Mat2 := diag Z8.one Z8.iNeg
def T2 : Mat2 := diag Z8.one Z8.zeta
def Tdg2 : Mat2 := diag Z8.one Z8.zeta3Neg

/-- Scaled Hadamard matrix `√2 · H = [[1, 1], [1, -1]]`. -/
def H2Scaled : Mat2 := ⟨Z8.one, Z8.one, Z8.one, Z8.negOne⟩

-- Concrete involution theorems (decided directly from matrix entries)
theorem x_involutive : mul X2 X2 = I2 := by decide
theorem y_involutive : mul Y2 Y2 = I2 := by decide
theorem z_involutive : mul Z2 Z2 = I2 := by decide
theorem hadamard_involutive_scaled : mul H2Scaled H2Scaled = scale (Z8.ofInt 2) I2 := by decide

-- Clifford+T Z₈ group identities
theorem t_squared_is_s : mul T2 T2 = S2 := by decide
theorem s_squared_is_z : mul S2 S2 = Z2 := by decide
theorem t_fourth_is_z : mul (mul T2 T2) (mul T2 T2) = Z2 := by decide
theorem t_eighth_is_identity :
    mul (mul (mul T2 T2) (mul T2 T2)) (mul (mul T2 T2) (mul T2 T2)) = I2 := by decide
theorem t_tdg_is_identity : mul T2 Tdg2 = I2 := by decide
theorem s_sdg_is_identity : mul S2 Sdg2 = I2 := by decide
theorem sdg_z_is_s : mul Sdg2 Z2 = S2 := by decide
theorem tdg_squared_is_sdg : mul Tdg2 Tdg2 = Sdg2 := by decide

-- Single-qubit commutation and Clifford conjugation identities
theorem z_s_commute : mul Z2 S2 = mul S2 Z2 := by decide
theorem hadamard_conjugates_x_to_z : mul (mul H2Scaled X2) H2Scaled = scale (Z8.ofInt 2) Z2 := by decide
theorem hadamard_conjugates_z_to_x : mul (mul H2Scaled Z2) H2Scaled = scale (Z8.ofInt 2) X2 := by decide
theorem s_conjugates_x_to_y : mul (mul S2 X2) Sdg2 = Y2 := by decide
theorem s_conjugates_y_to_neg_x : mul (mul S2 Y2) Sdg2 = scale Z8.negOne X2 := by decide
theorem h_x_non_commute : mul H2Scaled X2 ≠ mul X2 H2Scaled := by decide

/-- Canonical discrete Clifford+T phase group ℤ₈ indexing `I, T, S, T·S, Z, Z·T, S†, T†`. -/
def phaseZ8 : Fin 8 → Mat2
  | ⟨0, _⟩ => I2
  | ⟨1, _⟩ => T2
  | ⟨2, _⟩ => S2
  | ⟨3, _⟩ => mul T2 S2
  | ⟨4, _⟩ => Z2
  | ⟨5, _⟩ => mul Z2 T2
  | ⟨6, _⟩ => Sdg2
  | ⟨7, _⟩ => Tdg2

/-- Universal group homomorphism for all 64 pairs `(a, b) ∈ ℤ₈ × ℤ₈`. -/
theorem phaseZ8_add_all :
    ∀ a b : Fin 8,
      mul (phaseZ8 a) (phaseZ8 b) = phaseZ8 ⟨(a.val + b.val) % 8, Nat.mod_lt _ (by decide)⟩ := by
  decide

/-- Universal diagonal commutativity: EVERY pair of diagonal operators over ℤ[ζ₈] commutes. -/
theorem diag_comm (a0 a1 b0 b1 : Z8) :
    mul (diag a0 a1) (diag b0 b1) = mul (diag b0 b1) (diag a0 a1) := by
  simp only [mul, diag, Z8.mul_zero, Z8.add_zero, Z8.mul_comm]

theorem mul_one (A : Mat2) : mul A I2 = A := by
  cases A
  simp [mul, I2, diag, Z8.mul_zero, Z8.mul_one, Z8.add_zero, Z8.zero_add]

theorem one_mul (A : Mat2) : mul I2 A = A := by
  cases A
  simp [mul, I2, diag, Z8.zero_mul, Z8.one_mul, Z8.add_zero, Z8.zero_add]

theorem mul_assoc (A B C : Mat2) : mul (mul A B) C = mul A (mul B C) := by
  cases A; cases B; cases C
  simp only [mul, Mat2.mk.injEq]
  refine ⟨?_, ?_, ?_, ?_⟩ <;>
    simp only [Z8.left_distrib, Z8.right_distrib, Z8.mul_assoc] <;>
    exact Z8.add_swap4 _ _ _ _

/-- Word evaluation over `List Mat2` (left-to-right gate sequence `g₀ · g₁ · ... · g_{k-1}`). -/
def evalWord : List Mat2 → Mat2
  | [] => I2
  | g :: gs => mul g (evalWord gs)

theorem evalWord_append (w1 w2 : List Mat2) :
    evalWord (w1 ++ w2) = mul (evalWord w1) (evalWord w2) := by
  induction w1 with
  | nil => simp [evalWord, one_mul]
  | cons g gs ih => simp [evalWord, ih, mul_assoc]

/-- Commutation lifts from single gates to arbitrary-length commuting gate lists. -/
theorem commute_evalWord (A : Mat2) (mid : List Mat2)
    (hcomm : ∀ g ∈ mid, mul A g = mul g A) :
    mul A (evalWord mid) = mul (evalWord mid) A := by
  induction mid with
  | nil => simp [evalWord, mul_one, one_mul]
  | cons g gs ih =>
    have hg : mul A g = mul g A := hcomm g (List.Mem.head gs)
    have hgs : mul A (evalWord gs) = mul (evalWord gs) A :=
      ih (fun x hx => hcomm x (List.Mem.tail g hx))
    simp only [evalWord]
    calc
      mul A (mul g (evalWord gs)) = mul (mul A g) (evalWord gs) := (mul_assoc A g _).symm
      _ = mul (mul g A) (evalWord gs) := by rw [hg]
      _ = mul g (mul A (evalWord gs)) := mul_assoc g A _
      _ = mul g (mul (evalWord gs) A) := by rw [hgs]
      _ = mul (mul g (evalWord gs)) A := (mul_assoc g _ A).symm

/-- Inductive soundness of the arbitrary-distance lookahead peephole pass:
    if `A` commutes with every intervening gate in `mid` and `A · A⁻¹ = I₂`,
    then canceling `(A, A⁻¹)` around `mid` preserves circuit unitarity for any depth. -/
theorem lookahead_cancel_word (A Ainv : Mat2) (mid : List Mat2)
    (hcomm : ∀ g ∈ mid, mul A g = mul g A)
    (hinv : mul A Ainv = I2) :
    evalWord (A :: (mid ++ [Ainv])) = evalWord mid := by
  simp only [evalWord, evalWord_append]
  rw [← mul_assoc A (evalWord mid) (mul Ainv I2)]
  rw [commute_evalWord A mid hcomm]
  rw [mul_assoc (evalWord mid) A (mul Ainv I2)]
  rw [mul_one Ainv, hinv, mul_one]

/-- Projective equality modulo global phase `±1` (`U = V` or `U = -V` in SU(2)). -/
def ProjectiveEq (A B : Mat2) : Prop := A = B ∨ A = NegI2.mul B

/-- Spinor phase at angle `k · π` for `Rz(θ) = diag(e^{-iθ/2}, e^{iθ/2})`:
    `Rz(π)` has entries `diag(-i, i)`, whose square is `Rz(2π) = -I ≠ I`,
    while its 4th power `Rz(4π) = I`. -/
def RzPi : Mat2 := diag Z8.iNeg Z8.iUnit

theorem rz_pi_squared_is_neg_identity : mul RzPi RzPi = NegI2 := by decide
theorem rz_two_pi_projective_identity : ProjectiveEq (mul RzPi RzPi) I2 := Or.inr (by decide)
theorem rz_four_pi_strict_identity :
    mul (mul RzPi RzPi) (mul RzPi RzPi) = I2 := by decide

end Mat2

/-- Exact 4×4 two-qubit operator stored as row-major 4-tuples over ℤ[ζ₈]. -/
abbrev Row4 := Z8 × Z8 × Z8 × Z8

structure Mat4 where
  r0 : Row4
  r1 : Row4
  r2 : Row4
  r3 : Row4
  deriving DecidableEq, Repr

namespace Mat4

def rowDot (r : Row4) (c : Row4) : Z8 :=
  Z8.add (Z8.add (Z8.mul r.1 c.1) (Z8.mul r.2.1 c.2.1))
         (Z8.add (Z8.mul r.2.2.1 c.2.2.1) (Z8.mul r.2.2.2 c.2.2.2))

def col (M : Mat4) (idx : Fin 4) : Row4 :=
  match idx with
  | ⟨0, _⟩ => (M.r0.1, M.r1.1, M.r2.1, M.r3.1)
  | ⟨1, _⟩ => (M.r0.2.1, M.r1.2.1, M.r2.2.1, M.r3.2.1)
  | ⟨2, _⟩ => (M.r0.2.2.1, M.r1.2.2.1, M.r2.2.2.1, M.r3.2.2.1)
  | ⟨3, _⟩ => (M.r0.2.2.2, M.r1.2.2.2, M.r2.2.2.2, M.r3.2.2.2)

def mulRow (r : Row4) (B : Mat4) : Row4 :=
  (rowDot r (col B ⟨0, by omega⟩),
   rowDot r (col B ⟨1, by omega⟩),
   rowDot r (col B ⟨2, by omega⟩),
   rowDot r (col B ⟨3, by omega⟩))

def mul (A B : Mat4) : Mat4 :=
  ⟨mulRow A.r0 B, mulRow A.r1 B, mulRow A.r2 B, mulRow A.r3 B⟩

def z0 := Z8.zero
def z1 := Z8.one
def zm := Z8.negOne

def I4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, z1, z0, z0),
    (z0, z0, z1, z0),
    (z0, z0, z0, z1) ⟩

def CX4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, z1, z0, z0),
    (z0, z0, z0, z1),
    (z0, z0, z1, z0) ⟩

def CZ4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, z1, z0, z0),
    (z0, z0, z1, z0),
    (z0, z0, z0, zm) ⟩

def SWAP4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, z0, z1, z0),
    (z0, z1, z0, z0),
    (z0, z0, z0, z1) ⟩

def ZCtrl4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, z1, z0, z0),
    (z0, z0, zm, z0),
    (z0, z0, z0, zm) ⟩

def XTgt4 : Mat4 :=
  ⟨ (z0, z1, z0, z0),
    (z1, z0, z0, z0),
    (z0, z0, z0, z1),
    (z0, z0, z1, z0) ⟩

def ZTgt4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, zm, z0, z0),
    (z0, z0, z1, z0),
    (z0, z0, z0, zm) ⟩

def XCtrl4 : Mat4 :=
  ⟨ (z0, z0, z1, z0),
    (z0, z0, z0, z1),
    (z1, z0, z0, z0),
    (z0, z1, z0, z0) ⟩

def XBoth4 : Mat4 :=
  ⟨ (z0, z0, z0, z1),
    (z0, z0, z1, z0),
    (z0, z1, z0, z0),
    (z1, z0, z0, z0) ⟩

def ZBoth4 : Mat4 :=
  ⟨ (z1, z0, z0, z0),
    (z0, zm, z0, z0),
    (z0, z0, zm, z0),
    (z0, z0, z0, z1) ⟩

theorem cx_involutive : mul CX4 CX4 = I4 := by decide
theorem cz_involutive : mul CZ4 CZ4 = I4 := by decide
theorem swap_involutive : mul SWAP4 SWAP4 = I4 := by decide
theorem cx_z_ctrl_commute : mul CX4 ZCtrl4 = mul ZCtrl4 CX4 := by decide
theorem cx_x_tgt_commute : mul CX4 XTgt4 = mul XTgt4 CX4 := by decide
theorem cx_x_ctrl_non_commute : mul CX4 XCtrl4 ≠ mul XCtrl4 CX4 := by decide
theorem cx_z_tgt_non_commute : mul CX4 ZTgt4 ≠ mul ZTgt4 CX4 := by decide
theorem cx_conjugates_x_ctrl : mul (mul CX4 XCtrl4) CX4 = XBoth4 := by decide
theorem cx_conjugates_z_tgt : mul (mul CX4 ZTgt4) CX4 = ZBoth4 := by decide
theorem cz_z_ctrl_commute : mul CZ4 ZCtrl4 = mul ZCtrl4 CZ4 := by decide
theorem cz_z_tgt_commute : mul CZ4 ZTgt4 = mul ZTgt4 CZ4 := by decide
theorem swap_cz_commute : mul SWAP4 CZ4 = mul CZ4 SWAP4 := by decide

end Mat4

/-- Exact 8×8 three-qubit operator over `Fin 8 → Fin 8 → Z8`. -/
structure Mat8 where
  entry : Fin 8 → Fin 8 → Z8

namespace Mat8

private def f0 : Fin 8 := ⟨0, by decide⟩
private def f1 : Fin 8 := ⟨1, by decide⟩
private def f2 : Fin 8 := ⟨2, by decide⟩
private def f3 : Fin 8 := ⟨3, by decide⟩
private def f4 : Fin 8 := ⟨4, by decide⟩
private def f5 : Fin 8 := ⟨5, by decide⟩
private def f6 : Fin 8 := ⟨6, by decide⟩
private def f7 : Fin 8 := ⟨7, by decide⟩

def mul (A B : Mat8) : Mat8 :=
  ⟨fun i j =>
    let s01 := Z8.add (Z8.mul (A.entry i f0) (B.entry f0 j)) (Z8.mul (A.entry i f1) (B.entry f1 j))
    let s23 := Z8.add (Z8.mul (A.entry i f2) (B.entry f2 j)) (Z8.mul (A.entry i f3) (B.entry f3 j))
    let s45 := Z8.add (Z8.mul (A.entry i f4) (B.entry f4 j)) (Z8.mul (A.entry i f5) (B.entry f5 j))
    let s67 := Z8.add (Z8.mul (A.entry i f6) (B.entry f6 j)) (Z8.mul (A.entry i f7) (B.entry f7 j))
    Z8.add (Z8.add s01 s23) (Z8.add s45 s67)⟩

def I8 : Mat8 :=
  ⟨fun i j => if i.val == j.val then Z8.one else Z8.zero⟩

/-- Three-qubit Toffoli (`CCX(0, 1, 2)`): flips `q₂` iff `q₀ = 1` and `q₁ = 1` (basis 6 ↔ 7). -/
def CCX8 : Mat8 :=
  ⟨fun i j =>
    let tgt :=
      if i.val == 6 then 7
      else if i.val == 7 then 6
      else i.val
    if tgt == j.val then Z8.one else Z8.zero⟩

/-- `CNOT(0, 1)` on 3 qubits (`q₀` control, `q₁` target): swaps (4↔6) and (5↔7). -/
def CX01_8 : Mat8 :=
  ⟨fun i j =>
    let tgt := if i.val >= 4 then i.val ^^^ 2 else i.val
    if tgt == j.val then Z8.one else Z8.zero⟩

/-- `CNOT(0, 2)` on 3 qubits (`q₀` control, `q₂` target): swaps (4↔5) and (6↔7). -/
def CX02_8 : Mat8 :=
  ⟨fun i j =>
    let tgt := if i.val >= 4 then i.val ^^^ 1 else i.val
    if tgt == j.val then Z8.one else Z8.zero⟩

/-- `CNOT(1, 0)` on 3 qubits (`q₁` control, `q₀` target): swaps if bit 1 is 1 (`(i/2)%2 = 1`). -/
def CX10_8 : Mat8 :=
  ⟨fun i j =>
    let tgt := if (i.val / 2) % 2 == 1 then i.val ^^^ 4 else i.val
    if tgt == j.val then Z8.one else Z8.zero⟩

/-- `CNOT(2, 0)` on 3 qubits (`q₂` control, `q₀` target): swaps if bit 2 is 1 (`i % 2 = 1`). -/
def CX20_8 : Mat8 :=
  ⟨fun i j =>
    let tgt := if i.val % 2 == 1 then i.val ^^^ 4 else i.val
    if tgt == j.val then Z8.one else Z8.zero⟩

def ZCtrl0_8 : Mat8 :=
  ⟨fun i j => if i.val == j.val then (if i.val >= 4 then Z8.negOne else Z8.one) else Z8.zero⟩

def ZCtrl1_8 : Mat8 :=
  ⟨fun i j => if i.val == j.val then (if (i.val / 2) % 2 == 1 then Z8.negOne else Z8.one) else Z8.zero⟩

def XTgt2_8 : Mat8 :=
  ⟨fun i j => if (i.val ^^^ 1) == j.val then Z8.one else Z8.zero⟩

theorem ccx_involutive : ∀ i j : Fin 8, (mul CCX8 CCX8).entry i j = I8.entry i j := by decide
theorem ccx_z_c0_commute : ∀ i j : Fin 8, (mul CCX8 ZCtrl0_8).entry i j = (mul ZCtrl0_8 CCX8).entry i j := by decide
theorem ccx_z_c1_commute : ∀ i j : Fin 8, (mul CCX8 ZCtrl1_8).entry i j = (mul ZCtrl1_8 CCX8).entry i j := by decide
theorem ccx_x_t2_commute : ∀ i j : Fin 8, (mul CCX8 XTgt2_8).entry i j = (mul XTgt2_8 CCX8).entry i j := by decide
theorem cx_shared_ctrl_commute : ∀ i j : Fin 8, (mul CX01_8 CX02_8).entry i j = (mul CX02_8 CX01_8).entry i j := by decide
theorem cx_shared_tgt_commute : ∀ i j : Fin 8, (mul CX10_8 CX20_8).entry i j = (mul CX20_8 CX10_8).entry i j := by decide

/-- Overlapping `CZ(0,1)` and `CZ(1,2)` commute across shared wire `q₁`. -/
def CZ01_8 : Mat8 :=
  ⟨fun i j => if i.val == j.val then (if i.val >= 4 && (i.val / 2) % 2 == 1 then Z8.negOne else Z8.one) else Z8.zero⟩

def CZ12_8 : Mat8 :=
  ⟨fun i j => if i.val == j.val then (if (i.val / 2) % 2 == 1 && i.val % 2 == 1 then Z8.negOne else Z8.one) else Z8.zero⟩

theorem cz_overlap_commute : ∀ i j : Fin 8, (mul CZ01_8 CZ12_8).entry i j = (mul CZ12_8 CZ01_8).entry i j := by decide

end Mat8

end QIR.Formal
