# D14 narration + teleprompter cues (rfml5 1.2.1)

Numbers appear only as `{{field}}` from the recorded run's receipt (`measured.*`); read the value on screen, never from memory. Cues: `[RUN]` type/start, `[WAIT]` let output finish, `[SHOW]` hold the frame, `[PAUSE]` one beat.

1. [RUN] `bash fanout.sh`
   "One resident server answers forty-eight prompts, one after another. That is our baseline." [WAIT]
2. [SHOW] serial line
   "Serial: {{t1_ms}} milliseconds." [PAUSE]
3. [WAIT] four servers load; [SHOW] nvidia-smi table
   "Now four resident servers, four processes on the GPU. Same prompts, split four ways, one lane per port."
4. [SHOW] fan-out line
   "Four lanes: {{t4_ms}} milliseconds. Speedup {{speedup}}." [PAUSE]
5. [SHOW] contract line
   "The reducer sorts by id and hashes the bytes. Every id exactly once, and the merged hash matches the serial hash: {{sha256}}. Merge took {{merge_ms}} milliseconds."
6. [SHOW] Amdahl line
   "Back-solving Amdahl's law gives a serial fraction of {{serial_fraction}}. Four servers share one GPU, so the GPU is the part that does not parallelize. More lanes on this card will not fix that; another GPU or host would."
7. [SHOW] `contract: ... OK`, then the prompt returns
   "Contract held, and the traps have stopped every server. Nothing is left running."
