# Improve OpenHuman's recall over long agent histories

TinyMemory builds the recall pack OpenHuman injects before a turn. It is
measured on a frozen LongMemEval-V2 web subset: browsing trajectories are
ingested through TinyMemory into a local CortexDB, then questions are answered
from the pack alone. The brief is built only from the dev split: accuracy by
question type and a sample of failed dev questions with the model's answer and
the recall pack it was given. Gold answers and every test-split detail stay
out.

## Where we are

{{incumbent}}

## Earlier epochs

{{history}}

## Sample of failed dev questions

{{failures}}

## Ideas

{{ideas}}

## Your task

Find a general weakness in how TinyMemory gathers, ranks, excerpts or renders recalled
evidence into the pack that explains failures like these, and have the implementer fix
it with a unit test. Do not tune for these specific questions; the change is judged on
held-out questions and must make sense for any OpenHuman user with long agent histories.
