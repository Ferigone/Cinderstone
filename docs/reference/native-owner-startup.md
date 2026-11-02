# Retained native registry owner startup

One fresh native startup and STATUS/PING check passed on commit
`09da4e0185cf54330a7401deeff61fe7f329b07f`, tree
`93f95249be0ec6c8f86b42800fa339cd89862ff2`, with executable SHA256
`0ea5130fcfa819463df73e40f1e2667cb7548c1a222c60af1d358ab3388d2caa`.
The actual observations are in
[native-owner-startup.json](../validation/native-owner-startup.json).

The server reached its native listening event on an explicit loopback bind.
Its STATUS response identified Minecraft `1.21`, protocol `767`, with zero
online players; the subsequent PING payload was echoed exactly. The process
then followed its normal SIGINT shutdown and exited with code `0`. No PLAY
handshake, gameplay scenario, edited world/save input or restart was used.

In this recorded source, `fixtures()` must construct the retained block-tag
registry from the actual admitted configuration and the canonical inventory
and placement owners before startup reaches listening. The inventory owner
validates the original block/raw-ID, Property and per-state order certificate
during its existing OnceLock initialization. Reaching listening therefore
confirms that this mandatory constructor path returned successfully; this is
an inference from source control flow and the observed listening/status
events, rather than a separate runtime constructor instrumentation event.

The only launch used `--allow-offline --bind 127.0.0.1` and the explicit
`--loopback-service-keys` local public-key file. The actual key-source events
reported `loopback-file` and `network_refresh: false`; no official key-source
event was observed. The valid empty local key set supplied no certificate
or successful signed-certificate validation claim.

This initialization check supplies no gameplay evidence for concrete powder,
the clipping/admission path, imported block NBT, or complete Minecraft parity.
