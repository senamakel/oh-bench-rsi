# Mission

Make OpenHuman remember long agent working histories better. The measure is
the frozen LongMemEval-V2 web subset: an agent's past browsing trajectories
are stored through OpenHuman's TinyMemory lifecycle, and later questions about
those sites are answered from the recall pack OpenHuman injects before a turn
(about 1,200 tokens by default). A change counts only if it raises held-out
accuracy *and* is a sound improvement for any OpenHuman user.
