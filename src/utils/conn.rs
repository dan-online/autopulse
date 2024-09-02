pub fn get_conn(
    pool: &diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>> {
    pool.get()
        .expect("Failed to get database connection from pool")
}
