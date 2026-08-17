import Config

config :rekto, :word_type, :u64

config :krebs, :extra_mcp_tools, [
  Neoplasm.MCP.ListWatches,
  Neoplasm.MCP.AddWatch,
  Neoplasm.MCP.RemoveWatch,
  Neoplasm.MCP.UpdateWatch,
  Neoplasm.MCP.ListResults,
  Neoplasm.MCP.GenerateSchema,
  Neoplasm.MCP.LoadSchema
]
