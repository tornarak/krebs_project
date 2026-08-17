use rustler::{Encoder, Env, OwnedEnv, Term};

use crate::types::{ScanRegion, TypeRef};

#[derive(Debug)]
pub enum GuiEvent {
    ListProcessesRequested,
    AttachRequested(u32),
    DetachRequested,
    PrimitiveScanRequested {
        type_name: String,
        value: String,
        region: ScanRegion,
        next: bool,
    },
    PrimitivePreviewRequested {
        type_name: String,
        value: String,
    },
    SchemaPreviewRequested {
        schema: String,
        fields: Vec<(String, String)>,
    },
    IexRequested,
    HexDumpRequested(u64),
    MemoryLayoutRequested,
    CastPreviewRequested {
        addr: u64,
        type_ref: TypeRef,
    },
    WatchAddRequested {
        addr: u64,
        type_ref: TypeRef,
    },
    WatchRemoveRequested {
        addr: u64,
    },
    WatchInfoChanged {
        addr: u64,
        label: String,
        auto_refresh: bool,
        bare_map: bool,
    },
    QueryRequested {
        schema: String,
        fields: Vec<(String, String)>,
    },
    SchemaValidateRequested {
        module_name: String,
        fields: Vec<(String, String, u64, u64, String)>,
    },
    SchemaGenerateRequested {
        module_name: String,
        base_addr: u64,
        fields: Vec<(String, String, u64, u64, String)>,
    },
    SchemaLoadRequested {
        code: String,
    },
    SchemaSaveRequested {
        code: String,
        path: String,
    },
}

pub fn send_event(event: GuiEvent) {
    let recipient = {
        let Ok(guard) = super::STATE.lock() else {
            return;
        };
        guard.as_ref().map(|ws| ws.recipient)
    };
    let Some(recipient) = recipient else { return };
    log::debug!("send_event: {:?}", event);
    let mut env = OwnedEnv::new();
    let result = env.send_and_clear(&recipient, |env| encode_event(env, event));
    log::debug!("send_event done: {:?}", result);
}

fn encode_event<'a>(env: Env<'a>, event: GuiEvent) -> Term<'a> {
    use rustler::types::tuple::make_tuple;
    let tag = crate::atoms::neoplasm().encode(env);
    match event {
        GuiEvent::ListProcessesRequested => make_tuple(
            env,
            &[tag, crate::atoms::list_processes_requested().encode(env)],
        ),
        GuiEvent::AttachRequested(pid) => make_tuple(
            env,
            &[
                tag,
                crate::atoms::attach_requested().encode(env),
                pid.encode(env),
            ],
        ),
        GuiEvent::DetachRequested => {
            make_tuple(env, &[tag, crate::atoms::detach_requested().encode(env)])
        }
        GuiEvent::IexRequested => {
            make_tuple(env, &[tag, crate::atoms::iex_requested().encode(env)])
        }
        GuiEvent::HexDumpRequested(addr) => make_tuple(
            env,
            &[
                tag,
                crate::atoms::hex_dump_requested().encode(env),
                addr.encode(env),
            ],
        ),
        GuiEvent::MemoryLayoutRequested => make_tuple(
            env,
            &[tag, crate::atoms::memory_layout_requested().encode(env)],
        ),
        GuiEvent::CastPreviewRequested { addr, type_ref } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::cast_preview_requested().encode(env),
                addr.encode(env),
                type_ref.encode(env),
            ],
        ),
        GuiEvent::WatchAddRequested { addr, type_ref } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::watch_add_requested().encode(env),
                addr.encode(env),
                type_ref.encode(env),
            ],
        ),
        GuiEvent::WatchRemoveRequested { addr } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::watch_remove_requested().encode(env),
                addr.encode(env),
            ],
        ),
        GuiEvent::WatchInfoChanged {
            addr,
            label,
            auto_refresh,
            bare_map,
        } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::watch_info_changed().encode(env),
                addr.encode(env),
                label.encode(env),
                auto_refresh.encode(env),
                bare_map.encode(env),
            ],
        ),
        GuiEvent::QueryRequested { schema, fields } => {
            let field_terms: Vec<Term<'a>> = fields
                .into_iter()
                .map(|(name, value)| make_tuple(env, &[name.encode(env), value.encode(env)]))
                .collect();
            make_tuple(
                env,
                &[
                    tag,
                    crate::atoms::query_requested().encode(env),
                    schema.encode(env),
                    field_terms.encode(env),
                ],
            )
        }
        GuiEvent::PrimitiveScanRequested {
            type_name,
            value,
            region,
            next,
        } => {
            let region_atom = match region {
                ScanRegion::Heap => crate::atoms::heap(),
                ScanRegion::Module => crate::atoms::module(),
            }
            .encode(env);
            make_tuple(
                env,
                &[
                    tag,
                    crate::atoms::primitive_scan_requested().encode(env),
                    type_name.encode(env),
                    value.encode(env),
                    region_atom,
                    next.encode(env),
                ],
            )
        }
        GuiEvent::PrimitivePreviewRequested { type_name, value } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::primitive_preview_requested().encode(env),
                type_name.encode(env),
                value.encode(env),
            ],
        ),
        GuiEvent::SchemaPreviewRequested { schema, fields } => {
            let field_terms: Vec<Term<'a>> = fields
                .into_iter()
                .map(|(name, value)| make_tuple(env, &[name.encode(env), value.encode(env)]))
                .collect();
            make_tuple(
                env,
                &[
                    tag,
                    crate::atoms::schema_preview_requested().encode(env),
                    schema.encode(env),
                    field_terms.encode(env),
                ],
            )
        }
        GuiEvent::SchemaValidateRequested {
            module_name,
            fields,
        } => {
            let field_terms: Vec<Term<'a>> = fields
                .into_iter()
                .map(|(name, typ, offset, size, constraints)| {
                    make_tuple(
                        env,
                        &[
                            name.encode(env),
                            typ.encode(env),
                            offset.encode(env),
                            size.encode(env),
                            constraints.encode(env),
                        ],
                    )
                })
                .collect();
            make_tuple(
                env,
                &[
                    tag,
                    crate::atoms::schema_validate_requested().encode(env),
                    module_name.encode(env),
                    field_terms.encode(env),
                ],
            )
        }
        GuiEvent::SchemaGenerateRequested {
            module_name,
            base_addr,
            fields,
        } => {
            let field_terms: Vec<Term<'a>> = fields
                .into_iter()
                .map(|(name, typ, offset, size, constraints)| {
                    make_tuple(
                        env,
                        &[
                            name.encode(env),
                            typ.encode(env),
                            offset.encode(env),
                            size.encode(env),
                            constraints.encode(env),
                        ],
                    )
                })
                .collect();
            make_tuple(
                env,
                &[
                    tag,
                    crate::atoms::schema_generate_requested().encode(env),
                    module_name.encode(env),
                    base_addr.encode(env),
                    field_terms.encode(env),
                ],
            )
        }
        GuiEvent::SchemaLoadRequested { code } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::schema_load_requested().encode(env),
                code.encode(env),
            ],
        ),
        GuiEvent::SchemaSaveRequested { code, path } => make_tuple(
            env,
            &[
                tag,
                crate::atoms::schema_save_requested().encode(env),
                code.encode(env),
                path.encode(env),
            ],
        ),
    }
}
