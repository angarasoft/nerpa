# A configuration for a live run. The nodes it names must match the [nodes.*]
# tables of the fleet it is run with.
#
#     cargo xtask live --config main.star --fleet fleet.toml

# One file per machine, written as root through the installed executor.
for host in group("live"):
    resource("os:file.motd@%s" % host, {
        "path": "/etc/motd",
        "content": "managed by nerpa\n",
    })

# A service that reacts to a file: when the file changes, the service is
# restarted (roused). Only a service can react — a package cannot.
conf = resource("os:file.chrony@app-01", {
    "path": "/etc/chrony/chrony.conf",
    "content": "server 127.0.0.1 iburst\n",
})
resource("os:service.chronyd@app-01", {"running": True, "enabled": True}, reacts_to = [conf])
