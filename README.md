# e-routing
A fast routing system (using Bellman-Ford) in Rust.

Supports loading from OSRM data, or from a SQL database (at the moment, Postgres).

Run with (second line is example):
```
cargo run port database_user database_password database_name ways_vert_table_name ways_table_name forward_cost_column backward_cost_column
cargo run 9000 dominik 123456 ebikes ways_vertices_pgr ways v25_rec_wh v25_rec_r_wh
```

Build with (second line is how to start on Windows; make sure to be in the right directory, as the implementation uses the current directory to look for index.html, i.e., under src/static):
```
cargo build --release
target\release\bellman_osm.exe 9000 dominik 123456 ebikes ways_vertices_pgr ways v25_rec_wh v25_rec_r_wh
```

Then, open a browser and point it at http://127.0.0.1:9000, click on the map, or use the endpoints http://127.0.0.1:9000/api/route, http://127.0.0.1:9000/api/route-using-ids and http://127.0.0.1:9000/api/reachability. 

These three endpoints accept parameters as follows:

http://127.0.0.1:9000/api/route?source-lon=8.54564666748047&source-lat=47.407295617526366&target-lon=8.531398773193361&target-lat=47.366617842193385

http://127.0.0.1:9000/api/route-using-ids?source-id=1&target-id=5

http://127.0.0.1:9000/api/reachability?source-lon=8.50170135498047&source-lat=47.37429091011091&capacity=50.0
