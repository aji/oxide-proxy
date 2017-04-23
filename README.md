# oxide-proxy (unfinished)

This program listens for IRC connections and proxies them to an IRC server.

Feature status:

* [ ] Upgrade to `migrate`
* [ ] TLS termination

## Upgrade to `migrate`

oxide-proxy will attempt to upgrade connections to support the IRCv3 `migrate`
client capability. That is, if the receiving server supports `migrate` and the
client does not request it, then oxide-proxy will request `migrate` on behalf
of the client and transparently implement the migration protocol.

(Note: `migrate` is not yet an official IRCv3 standard.)

## TLS termination

oxide-proxy can listen for TLS connections and "downgrade" them to plaintext
to be proxied to the target server.
