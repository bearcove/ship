interface User {
  id: string;
  name: string;
  email: string;
  role: "admin" | "user" | "guest";
}

interface ApiResponse<T> {
  data: T;
  status: number;
  message?: string;
}

type UserRole = User["role"];

class ApiClient {
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  setToken(token: string): void {
    this.token = token;
  }

  async getUser(id: string): Promise<ApiResponse<User>> {
    const response = await fetch(`${this.baseUrl}/users/${id}`, {
      headers: this.getHeaders(),
    });
    return response.json();
  }

  async listUsers(): Promise<ApiResponse<User[]>> {
    const response = await fetch(`${this.baseUrl}/users`, {
      headers: this.getHeaders(),
    });
    return response.json();
  }

  async createUser(user: Omit<User, "id">): Promise<ApiResponse<User>> {
    const response = await fetch(`${this.baseUrl}/users`, {
      method: "POST",
      headers: this.getHeaders(),
      body: JSON.stringify(user),
    });
    return response.json();
  }

  async deleteUser(id: string): Promise<ApiResponse<void>> {
    const response = await fetch(`${this.baseUrl}/users/${id}`, {
      method: "DELETE",
      headers: this.getHeaders(),
    });
    return response.json();
  }

  private getHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.token) {
      headers["Authorization"] = `Bearer ${this.token}`;
    }
    return headers;
  }
}

function isAdmin(user: User): boolean {
  return user.role === "admin";
}

function formatUserName(user: User): string {
  return `${user.name} <${user.email}>`;
}

const API_VERSION = "v2";
const DEFAULT_PAGE_SIZE = 25;

export { ApiClient, isAdmin, formatUserName, API_VERSION, DEFAULT_PAGE_SIZE };
export type { User, ApiResponse, UserRole };
