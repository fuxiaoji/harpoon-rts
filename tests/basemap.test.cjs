const test = require('node:test');
const assert = require('node:assert/strict');
require('../web/basemap-index.js');
test('basemaps resolve geographic bounds for old archives and reject synthetic geography', () => {
  const path = globalThis.HarpoonBasemaps.path;
  assert.equal(path({west:126,south:16,east:138,north:24,image:'maps/west-pacific-blue-marble.jpg'}),'maps/csg-126-138-blue-marble.jpg');
  assert.equal(path({west:118,south:18,east:134,north:30}),'maps/west-pacific-blue-marble.jpg');
  assert.equal(path({west:0,south:0,east:10,north:10,image:'maps/west-pacific-blue-marble.jpg'}),null);
  assert.equal(path({synthetic:true,image:'maps/west-pacific-blue-marble.jpg'}),null);
});
