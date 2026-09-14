SCORES:
{% for item in score_result %}
- {{ item.agent | replace("score_", "") }}: {{ item.score }}/5 — {{ item.reasoning }}
{% endfor %}

DIFF:
{{ diff }}

Focus your critique on the lowest-scoring categories. For each issue identify the exact location, what is wrong and why it matters, and how to fix it.