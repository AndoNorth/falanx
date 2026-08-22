DIFF:
{{ diff }}

Do these changes introduce performance problems?
Score 5 if the changes are efficient or neutral. Score lower only if the changes actively introduce regressions.

- Do the changes add unnecessary allocations or copies?
- Do the changes introduce worse algorithmic complexity where better is feasible?
- Do the changes block where async would be appropriate?

Respond with only the following JSON — no preamble, no explanation, no other text:
{"score": <integer 1-5>, "reasoning": "<max 80 words>"}
