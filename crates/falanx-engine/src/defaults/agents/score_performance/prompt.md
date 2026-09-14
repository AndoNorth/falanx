DIFF:
{{ diff }}

Do these changes introduce performance problems?
Score 5 if the changes are efficient or neutral. Score lower only if the changes actively introduce regressions.

- Do the changes add unnecessary allocations or copies?
- Do the changes introduce worse algorithmic complexity where better is feasible?
- Do the changes block where async would be appropriate?