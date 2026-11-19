; ModuleID = 'lr2_obfuscated'
source_filename = "lr2_obfuscated"

define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__x__body(ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__rz__body(double 0x3FF921FAFC8B007A, ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__x__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__x__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__cnot__body(ptr null, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__cnot__body(ptr null, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__cnot__body(ptr null, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__rz__body(double 6.283180e-01, ptr null)
  call void @__quantum__qis__rz__body(double -6.283180e-01, ptr null)
  call void @__quantum__qis__rz__body(double 0x3FE921FB54442D18, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__h__body(ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__y__body(ptr inttoptr (i64 2 to ptr))
  call void @__quantum__qis__mz__body(ptr null, ptr null)
  call void @__quantum__qis__mz__body(ptr inttoptr (i64 1 to ptr), ptr inttoptr (i64 1 to ptr))
  ret void
}

declare void @__quantum__qis__h__body(ptr)

declare void @__quantum__qis__x__body(ptr)

declare void @__quantum__qis__rz__body(double, ptr)

declare void @__quantum__qis__cnot__body(ptr, ptr)

declare void @__quantum__qis__y__body(ptr)

declare void @__quantum__qis__mz__body(ptr, ptr writeonly) #1

attributes #0 = { "entry_point" "output_labeling_schema" "qir_profiles"="custom" "required_num_qubits"="3" "required_num_results"="2" }
attributes #1 = { "irreversible" }

!llvm.module.flags = !{!0, !1, !2, !3}

!0 = !{i32 1, !"qir_major_version", i32 2}
!1 = !{i32 7, !"qir_minor_version", i32 0}
!2 = !{i32 1, !"dynamic_qubit_management", i1 false}
!3 = !{i32 1, !"dynamic_result_management", i1 false}
