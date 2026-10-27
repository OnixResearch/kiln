let Cobs = import "../web-cobs.ncl" in
let Valid = import "web-cobs-inputs.ncl" in
Cobs (
  Valid
  & {
    repositories | force =
      std.array.map
        (fun repository => repository & { git_directory | force = "relative/cobs.git" })
        Valid.repositories,
  }
)
