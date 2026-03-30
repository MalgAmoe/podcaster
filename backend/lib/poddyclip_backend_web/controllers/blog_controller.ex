defmodule PoddyclipBackendWeb.BlogController do
  use PoddyclipBackendWeb, :controller

  alias PoddyclipBackend.Blog

  def index(conn, _params) do
    locale = conn.assigns[:locale] || "en"
    posts = Blog.list_posts(locale)

    conn
    |> assign(:posts, posts)
    |> assign(:page_title, gettext("Blog"))
    |> put_layout(false)
    |> render(:index)
  end

  def show(conn, %{"slug" => slug}) do
    locale = conn.assigns[:locale] || "en"

    case Blog.get_post(slug, locale) do
      nil ->
        conn
        |> put_status(:not_found)
        |> put_view(PoddyclipBackendWeb.ErrorHTML)
        |> render(:"404")

      post ->
        conn
        |> assign(:post, post)
        |> assign(:page_title, post.title)
        |> assign(:meta_description, post.description)
        |> assign(:meta_image, post.image)
        |> put_layout(false)
        |> render(:show)
    end
  end
end
