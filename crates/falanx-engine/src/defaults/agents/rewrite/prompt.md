ISSUES:
{{ review_result | tojson }}

DIFF:
{{ diff }}

Apply only the changes needed to address each issue — nothing more. For each change include the exact original text, the revised text, and which issue it addresses.

Respond with only a JSON array — no preamble, no explanation, no other text. Return [] if no changes are needed:
[{"original": "exact original text", "revised": "replacement text", "issue_ref": "location from issue"}]
