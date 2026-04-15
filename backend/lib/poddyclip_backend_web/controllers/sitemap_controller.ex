defmodule PoddyclipBackendWeb.SitemapController do
  use PoddyclipBackendWeb, :controller

  @base_url "https://munchycow.com"

  @static_pages [
    {"/", "weekly", "1.0"},
    {"/help", "monthly", "0.5"},
    {"/terms", "yearly", "0.3"},
    {"/privacy", "yearly", "0.3"},
    {"/legal", "yearly", "0.3"}
  ]

  def index(conn, _params) do
    xml = build_sitemap()

    conn
    |> put_resp_content_type("application/xml")
    |> send_resp(200, xml)
  end

  defp build_sitemap do
    urls =
      Enum.map(@static_pages, fn {path, changefreq, priority} ->
        url_entry(path, changefreq, priority)
      end)

    """
    <?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
    #{Enum.join(urls, "\n")}
    </urlset>
    """
    |> String.trim()
  end

  defp url_entry(path, changefreq, priority) do
    """
      <url>
        <loc>#{@base_url}#{path}</loc>
        <changefreq>#{changefreq}</changefreq>
        <priority>#{priority}</priority>
      </url>
    """
    |> String.trim_trailing()
  end
end
