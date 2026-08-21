{
  outputs =
    { self, ... }:
    let
      System = "x86_64-linux";
      JobA = (derivation {
        name = "kiln-job-a";
        system = System;
        builder = "/bin/sh";
        args = [
          "-c"
          "echo job-a > $out"
        ];
      }) // {
        meta = {
          description = "Kiln positive cohort job";
        };
      };
    in
    {
      jobs = {
        job-a = JobA;
        job-b = (derivation {
          name = "kiln-job-b";
          system = System;
          builder = "/bin/sh";
          args = [
            "-c"
            "echo ${JobA} > $out"
          ];
        }) // {
          meta = {
            description = "Kiln dependent cohort job";
          };
        };
        broken = throw "Kiln fixture evaluation error";
      };
      onix-only = {
        unsupported = derivation {
          name = builtins.wasm;
          system = System;
          builder = "/bin/sh";
        };
      };
    };
}
