def test_v0_imports():
    from debmagic.v0 import dh

    preset_instance = dh.Preset()
    assert preset_instance is not None
