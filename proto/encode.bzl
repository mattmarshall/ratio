"""Use the configured protoc and full descriptor closure for operator policy."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")

_PROTO_TOOLCHAIN = "@protobuf//bazel/private:proto_toolchain_type"

def _proto_encoder_impl(ctx):
    protoc = ctx.toolchains[_PROTO_TOOLCHAIN].proto.proto_compiler
    descriptors = ctx.attr.proto[ProtoInfo].transitive_descriptor_sets
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    # All runfile paths are rooted in the generated executable's runfiles tree.
    # stdin/stdout stay untouched so no credentials or policy appear in logs.
    ctx.actions.write(script, """#!/usr/bin/env bash
set -euo pipefail
cd "${RUNFILES_DIR:-$0.runfiles}/_main"
exec "%s" --descriptor_set_in="%s" --encode=%s
""" % (protoc.executable.short_path, ":".join([f.short_path for f in descriptors.to_list()]), ctx.attr.message), is_executable = True)
    return [DefaultInfo(
        executable = script,
        runfiles = ctx.runfiles(files = [protoc.executable], transitive_files = descriptors),
    )]

proto_encoder = rule(
    implementation = _proto_encoder_impl,
    attrs = {
        "proto": attr.label(providers = [ProtoInfo], mandatory = True),
        "message": attr.string(mandatory = True),
    },
    toolchains = [_PROTO_TOOLCHAIN],
    executable = True,
)
