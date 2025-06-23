# Auditor Instructions

This repository contains the telcoin-network Rust code and Solidity tn-contracts. Follow these guidelines when adding or modifying code, or creating audit reports.

- **Findings Formatting**: When submitting a finding, include Severity (High, Medium, Low, Information), Likelihood score, Impact score, and a short Finding Title.
- Use the following template for findings:
  ## Summary
  A short summary of the issue, keep it brief.

  ## Finding Description
  A more detailed explanation of the issue. Poorly written or incorrect findings may result in rejection and a decrease of reputation score.

  Describe which security guarantees it breaks and how it breaks them. If this bug does not automatically happen, showcase how a malicious input would propagate through the system to the part of the code where the issue occurs.

  ## Impact Explanation
  Elaborate on why you've chosen a particular impact assessment.

  ## Likelihood Explanation
  Explain how likely this is to occur and why.

  ## Proof of Concept
  A proof of concept is normally required for Critical, High and Medium submissions for reviewers under 80 reputation points. Please check the competition page for more details, otherwise your submission may be rejected by the judges.

  ## Recommendation
  How can the issue be fixed or solved. Preferably, you can also add a snippet of the fixed code here.

- **Testing Requirements**: After modifying code, run all provided tests.
- Use `cargo test --all` for the Rust code and `forge test` for Solidity contracts.
