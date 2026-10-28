(import "hosted-pipeline.ncl") {
  name = "Repository checks",
  triggers = ["manual", "push"],
  parameters = { message = "hello" },
  steps.greet = {
    parameters = { text = "hello" },
    step = {
      run = "printf '%s\\n' '${{ parameters.text }}'",
      shell = "bash",
    },
  },
  templates.check = {
    runner = "linux-reviewed",
    steps = [
      { kind = "use", step = "greet", parameters = { text = "${{ parameters.message }}" } },
      { kind = "run", run = "printf '%s\\n' '${{ matrix.version }}'", shell = "bash" },
    ],
  },
  jobs = {
    check = { template = "check", matrix = { version = ["stable", "next"] } },
    finish = {
      runner = "linux-reviewed",
      needs = ["check"],
      steps = [{ kind = "run", run = "printf 'checks succeeded\\n'", shell = "bash" }],
    },
  },
}
