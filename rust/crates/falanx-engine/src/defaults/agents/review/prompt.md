Review this diff. The current quality scores are:

{% for item in score_result %}
- {{ item.agent | replace("score_", "") }}: {{ item.score }}/5 — {{ item.reasoning }}
{% endfor %}

Focus your critique on the lowest-scoring categories. For each issue identify: the exact location, what is wrong and why it matters, and how to fix it.

Respond as JSON array only. Return [] if no issues found.
[{"location":"file:line","problem":"description of the issue","fix":"specific fix suggestion"}]

DIFF:
{{ diff }}
