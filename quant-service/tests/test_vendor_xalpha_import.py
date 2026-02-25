def test_vendor_xalpha_imports_without_optional_deps():
    import xalpha  # noqa: F401
    import xalpha.cons  # noqa: F401
    import xalpha.policy  # noqa: F401
    import xalpha.backtest  # noqa: F401
    import xalpha.trade  # noqa: F401
    import xalpha.multiple  # noqa: F401

