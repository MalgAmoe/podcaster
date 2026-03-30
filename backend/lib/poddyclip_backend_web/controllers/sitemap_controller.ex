defmodule PoddyclipBackendWeb.SitemapController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Blog

  @base_url "https://munchycow.com"

  @static_pages [
    {"/", "weekly", "1.0"},
    {"/blog", "weekly", "0.8"},
    {"/help", "monthly", "0.5"},
    {"/terms", "yearly", "0.3"},
    {"/privacy", "yearly", "0.3"},
    {"/legal", "yearly", "0.3"}
  ]

  def index(conn, _params) do
    blog_slugs =
      Blog.all_posts()
      |> Enum.map(& &1.slug)
      |> Enum.uniq()

    post_dates =
      Blog.all_posts()
      |> Enum.group_by(& &1.slug)
      |> Map.new(fn {slug, posts} ->
        latest = posts |> Enum.max_by(& &1.date)
        {slug, latest.date}
      end)

    xml = build_sitemap(blog_slugs, post_dates)

    conn
    |> put_resp_content_type("application/xml")
    |> send_resp(200, xml)
  end

  defp build_sitemap(blog_slugs, post_dates) do
    urls =
      Enum.map(@static_pages, fn {path, changefreq, priority} ->
        url_entry(path, changefreq, priority)
      end) ++
        Enum.map(blog_slugs, fn slug ->
          lastmod = Map.get(post_dates, slug)
          url_entry("/blog/#{slug}", "monthly", "0.6", lastmod)
        end)

    """
    <?xml version="1.0" encoding="UTF-8"?>
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
    #{Enum.join(urls, "\n")}
    </urlset>
    """
    |> String.trim()
  end

  defp url_entry(path, changefreq, priority, lastmod \\ nil) do
    lastmod_tag =
      if lastmod do
        "\n    <lastmod>#{Date.to_iso8601(lastmod)}</lastmod>"
      else
        ""
      end

    """
      <url>
        <loc>#{@base_url}#{path}</loc>#{lastmod_tag}
        <changefreq>#{changefreq}</changefreq>
        <priority>#{priority}</priority>
      </url>
    """
    |> String.trim_trailing()
  end
end
