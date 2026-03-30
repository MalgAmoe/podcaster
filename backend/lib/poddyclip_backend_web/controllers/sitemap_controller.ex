defmodule PoddyclipBackendWeb.SitemapController do
  use PoddyclipBackendWeb, :controller

  @base_url "https://munchycow.com"
  @locales ["en"]

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
    <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
            xmlns:xhtml="http://www.w3.org/1999/xhtml">
    #{Enum.join(urls, "\n")}
    </urlset>
    """
    |> String.trim()
  end

  defp url_entry(path, changefreq, priority) do
    loc = locale_url("en", path)

    alternates =
      Enum.map(@locales, fn locale ->
        href = locale_url(locale, path)
        hreflang = if locale == "en", do: "en", else: locale
        ~s(    <xhtml:link rel="alternate" hreflang="#{hreflang}" href="#{href}"/>)
      end)
      |> Kernel.++([~s(    <xhtml:link rel="alternate" hreflang="x-default" href="#{loc}"/>)])
      |> Enum.join("\n")

    """
      <url>
        <loc>#{loc}</loc>
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
