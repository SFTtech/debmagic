# Making releases

First step is bumping the versions

```shell
uv version --package <package-name> --bump <major | minor | patch | beta | alpha> [--dry-run]
```

Currently we version all packages in tandem without separate release for our sub-packages

```shell
export BUMP=<bump-type>
uv version --package debmagic-common --bump $BUMP
uv version --package debmagic-api --bump $BUMP
uv version --package debmagic --bump $BUMP
```
