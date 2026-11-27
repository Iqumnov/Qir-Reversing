import Lake
open Lake DSL

package qir_formal where
  srcDir := "."

@[default_target]
lean_lib UnitaryEquivalence where
  roots := #[`UnitaryEquivalence]

