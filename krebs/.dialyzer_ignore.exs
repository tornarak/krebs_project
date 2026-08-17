# Warnings suppressed here are by design or unavoidable dialyzer limitations.
[
  # NIF stubs all contain `:erlang.nif_error(:nif_not_loaded)` as their body.
  # Dialyzer correctly infers that the function body always raises, making the
  # declared return type unreachable. This is intentional: the real return type
  # is provided by the loaded NIF at runtime.
  {"lib/krebs/nif.ex", :no_return},

  # Cascade from NIF stubs: functions that call NIFs are also inferred as no_return,
  # and the binary-vs-charlist NIF boundary contract is unresolvable without the loaded
  # NIF. At runtime, Rustler's decoder accepts both; dialyzer only sees the stub spec.
  {"lib/krebs/scan_pattern.ex", :no_return},
  {"lib/krebs/scan_pattern.ex", :invalid_contract},
  {"lib/krebs/scan_pattern.ex", :call},
  {"lib/krebs/repo.ex", :no_return},

  # Mix.Task behaviour and Mix.raise/1 are not fully visible to dialyzer from the
  # OTP PLT. These are valid at runtime; dialyzer simply lacks the type info.
  {"lib/mix/tasks/krebs.attach.ex", :callback_info_missing},
  {"lib/mix/tasks/krebs.attach.ex", :unknown_function}
]
