# Warnings suppressed here are by design or unavoidable dialyzer limitations.
[
  # word_size/0 is a compile-time macro that expands to a literal based on :word_type config.
  # The non-configured branches (:u64, fallthrough) are intentionally unreachable at compile time
  # and serve as a guard against misconfiguration. @dialyzer cannot annotate macros directly.
  {"lib/rekto/serialization.ex", :pattern_match},
  {"lib/rekto/serialization.ex", :pattern_match_cov}
]
