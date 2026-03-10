defmodule PoddyclipBackendWeb.SitemapController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Blog

  @base_url "https://munchycow.com"
  @locales ["en", "es", "fr", "it"]

  @static_pages [
    {"/", "weekly", "1.0"},
    {"/pricing", "monthly", "0.8"},
    {"/blog", "weekly", "0.8"},
    {"/help", "monthly", "0.5"},
    {"/terms", "yearly", "0.3"},
    {"/privacy", "yearly", "0.3"},
    {"/legal", "yearly", "0.3"}
  ]

  def index(conn, _params) do
    blog_posts =
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

    xml = build_sitemap(blog_posts, post_dates)

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
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
            xmlns:xhtml="http://www.w3.org/1999/xhtml">
    #{Enum.join(urls, "\n")}
    </urlset>
    """
    |> String.trim()
  end

  defp url_entry(path, changefreq, priority, lastmod \\ nil) do
    loc = locale_url("en", path)

    alternates =
      Enum.map(@locales, fn locale ->
        href = locale_url(locale, path)
        hreflang = if locale == "en", do: "en", else: locale
        ~s(    <xhtml:link rel="alternate" hreflang="#{hreflang}" href="#{href}"/>)
      end)
      |> Kernel.++([~s(    <xhtml:link rel="alternate" hreflang="x-default" href="#{loc}"/>)])
      |> Enum.join("\n")

    lastmod_tag =
      if lastmod do
        "\n    <lastmod>#{Date.to_iso8601(lastmod)}</lastmod>"
      else
        ""
      end

    """
      <url>
        <loc>#{loc}</loc>#{lastmod_tag}
        <changefreq>#{changefreq}</changefreq>
        <priority>#{priority}</priority>
    #{alternates}
      </url>
    """
    |> String.trim_trailing()
  end

  defp locale_url("en", path), do: "#{@base_url}#{path}"
  defp locale_url(locale, "/"), do: "#{@base_url}/#{locale}"
  defp locale_url(locale, path), do: "#{@base_url}/#{locale}#{path}"
end
