rustler::atoms! {
    ok,
    error,
    neoplasm,
    none,
    already_started,
    // Rekto primitive type atoms — mirror Rekto.Serialization primitive set
    bool,
    byte,
    i8,
    u8,
    u16,
    i16,
    u32,
    i32,
    u64,
    i64,
    f32,
    f64,
    word,
    // GUI → Elixir events
    list_processes_requested,
    attach_requested,
    detach_requested,
    scan_requested,
    primitive_scan_requested,
    primitive_preview_requested,
    schema_preview_requested,
    query_requested,
    iex_requested,
    window_closed,
    // scan regions
    heap,
    module,
    // scan status
    idle,
    scanning,
    done,
    // hex dump inspector events
    hex_dump_requested,
    memory_layout_requested,
    cast_preview_requested,
    // watched addresses
    watch_add_requested,
    watch_remove_requested,
    watch_info_changed,
    // schema builder events
    schema_validate_requested,
    schema_generate_requested,
    schema_load_requested,
    schema_save_requested,
    // memory region kinds
    other,
    // compound type constructors
    string_buffer,
    // schema field keys (FieldInfo / Schema.Info struct fields)
    name,
    data_type,
    fields,
    offset,
    size,
    points_to,
}
