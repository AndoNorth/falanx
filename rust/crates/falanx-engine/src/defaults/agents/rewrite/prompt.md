Fix the following issues in this diff. Apply only the changes needed to address each issue — nothing more.

For each change, include the exact original text, the revised text, and which issue it addresses.

Respond as JSON array only. Return [] if no changes are needed.
[{"original":"exact original text from diff","revised":"replacement text","issue_ref":"location from issue"}]

ISSUES:
{{ review_result | tojson }}

DIFF:
{{ diff }}
