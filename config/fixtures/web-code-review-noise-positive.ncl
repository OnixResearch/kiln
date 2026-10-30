# Display-only noise rules for lockfiles, vendored and generated paths.
(import "../web-code-review.ncl") (
  (import "web-code-review-inputs.ncl")
  & {
    review = {
      noise = [
        { id = "lockfiles", pattern = "**/Cargo.lock", reason = "Cargo lockfile churn is mechanical" },
        { id = "vendor", pattern = "vendor/**", reason = "Vendored third-party sources" },
        { id = "generated-policy", pattern = "cairn-policy/generated/**", reason = "Generated Cairn policy" },
        { id = "cairn-evidence", pattern = ".cairn/archive/**/evidence/**", reason = "Archived lifecycle evidence" },
      ],
    },
  }
)
