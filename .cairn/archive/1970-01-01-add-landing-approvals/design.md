## Decisions

Reviews are reduced to the latest state per actor because GitHub returns review
events rather than a current vote. The author is excluded by the observed PR
author, and a latest changes-requested result blocks regardless of approval
count. The page is bounded; incomplete review pagination is refused.

## Non-claims

Review state is GitHub-reported evidence, not authority to merge. This change
adds no approval, dismissal, merge or landing effect.
