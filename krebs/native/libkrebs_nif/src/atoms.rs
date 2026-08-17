atoms! {
    ok,
    error,

    // access levels
    read, read_write,

    // mem types
    heap, module,

    // KrebsError discriminants
    mem, cpp_std,

    // MemError discriminants
    windows, unix,

    // WinMemError variants
    read_failed, write_failed, process_access_denied, process_close_failed,
    process_already_closed, access_denied, memory_access_denied, invalid_handle,
    partial_copy, null_pointer, virtual_query_failed, module_enumeration_failed, misc,

    // UnixMemError variants
    proc_mem_read_failed, proc_mem_write_failed, proc_maps_open_failed,
    proc_maps_row_parse_error, proc_maps_addr_range_parse_failed,
    proc_maps_perms_parse_failed, process_open_failed,

    // StdError discriminants
    vcpp, gcc, common_string,

    // vcpp::Error
    string, vector,

    // vcpp::StringError
    short, long,

    // vcpp::ShortStringError
    zero_length, bad_alloc_size,

    // vcpp::LongStringError
    too_short, alloc_too_small, invalid_alloc_size, null_ptr,

    // vcpp::VectorError
    invalid_range, misaligned, io,

    // gcc::ShortStringError
    invalid_length,

    // gcc::LongStringError
    capacity_too_small,

    // CommonStringError
    buffer_length_mismatch, no_null_terminator, embedded_null_bytes, invalid_utf8,

    // ScannerError
    not_in_heap, not_in_module,

    // field-name atoms (keyword lists in error tuples)
    addr, size, code, pid, access_mask, copied, expected,
    got, alloc_size, needed, length, capacity,
    first_addr, last_addr, diff, element_size,
    buf_len, str_len, source, kind, field_count, row, input,

    // proc_map fields
    base, start, range_end, perms, mem_type, state,
    name,

    // backward-compat
    already_closed,

    // scan type (legacy)
    invalid_mem_type,

    // log bridge
    krebs_log,
    warning,
    info,
    debug,
    trace
}
