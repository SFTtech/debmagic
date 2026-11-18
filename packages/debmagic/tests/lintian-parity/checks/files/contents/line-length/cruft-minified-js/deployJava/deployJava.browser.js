(function(modules, cache, entry) {
  req(entry);
  function req(name) {
    if (cache[name]) return cache[name].exports;
    var m = cache[name] = {exports: {}};
    modules[name][0].call(m.exports, modRequire, m, m.exports, window);
    return m.exports;
    function modRequire(alias) {
      var id = modules[name][1][alias];
      if (!id) throw new Error("Cannot find module " + alias);
      return req(id);
    }
  }
})({0: [function(require,module,exports,global){
var deployJava=function(){};
/* simulate a long line */
var longline = 'XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX';

}, {}],}, {}, 0);
