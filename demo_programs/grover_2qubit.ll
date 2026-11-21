; ModuleID = 'grover_2qubit'
source_filename = "grover_2qubit"

define void @main() #0 {
entry:
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__cz__body(ptr null, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__x__body(ptr null)
  call void @__quantum__qis__x__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__cz__body(ptr null, ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__x__body(ptr null)
  call void @__quantum__qis__x__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__h__body(ptr null)
  call void @__quantum__qis__h__body(ptr inttoptr (i64 1 to ptr))
  call void @__quantum__qis__mz__body(ptr null, ptr null)
  call void @__quantum__qis__mz__body(ptr inttoptr (i64 1 to ptr), ptr inttoptr (i64 1 to ptr))
  ret void
}

attributes #0 = { "required_num_qubits"="2" "required_num_results"="2" }

