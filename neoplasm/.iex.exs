alias Krebs.{Scanner, HexDump, Repo, ScanPattern}
alias Rekto.Query

import Krebs.Scanner,
  only: [
    list_processes: 0,
    search_processes: 1,
    list_windows: 0,
    search_windows: 1,
    list_instances: 0,
    instance_names: 0,
    pid: 0,
    pid: 1,
    executable_name: 0,
    executable_name: 1,
    window_names: 0,
    window_names: 1,
    access_level: 0,
    access_level: 1,
    read: 2,
    read: 3,
    read_type: 2,
    read_type: 3,
    write: 2,
    write: 3,
    regions: 0,
    regions: 1,
    modules: 0,
    modules: 1,
    scan: 3,
    scan: 4,
    in_memory?: 2,
    in_memory?: 3,
    refresh_layout: 0,
    refresh_layout: 1
  ]

import Krebs.HexDump,
  only: [hex_dump: 2, hex_dump: 3, hex_dump: 4, type_dump: 3, type_dump: 4]

if Node.self() == :nonode@nohost, do: Node.start(:neoplasm, :shortnames)
