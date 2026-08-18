/**
 * NestJS controller handling user resource routes.
 */

import { CreateUserDto, UpdateUserDto, UserResponseDto } from './user.dto';
import { AuthGuard } from './auth.guard';

export function Controller(prefix?: string): ClassDecorator {
  return () => {};
}

export function Get(path?: string): MethodDecorator {
  return () => {};
}

export function Post(path?: string): MethodDecorator {
  return () => {};
}

export function Put(path?: string): MethodDecorator {
  return () => {};
}

export function Delete(path?: string): MethodDecorator {
  return () => {};
}

export function Body(): ParameterDecorator {
  return () => {};
}

export function Param(paramName?: string): ParameterDecorator {
  return () => {};
}

export function UseGuards(...guards: any[]): MethodDecorator & ClassDecorator {
  return () => {};
}

@Controller('users')
@UseGuards(AuthGuard)
export class UsersController {
  @Get()
  async findAll(): Promise<UserResponseDto[]> {
    return [
      {
        id: 'usr-1',
        username: 'alice',
        email: 'alice@example.com',
        displayName: 'Alice Architect',
        createdAt: new Date(),
      },
    ];
  }

  @Get(':id')
  async findOne(@Param('id') id: string): Promise<UserResponseDto> {
    return {
      id,
      username: 'bob',
      email: 'bob@example.com',
      displayName: 'Bob Builder',
      createdAt: new Date(),
    };
  }

  @Post()
  async create(@Body() createUserDto: CreateUserDto): Promise<UserResponseDto> {
    return {
      id: 'usr-new',
      username: createUserDto.username,
      email: createUserDto.email,
      displayName: createUserDto.displayName,
      createdAt: new Date(),
    };
  }

  @Put(':id')
  async update(
    @Param('id') id: string,
    @Body() updateUserDto: UpdateUserDto
  ): Promise<UserResponseDto> {
    return {
      id,
      username: 'updated-user',
      email: updateUserDto.email || 'user@example.com',
      displayName: updateUserDto.displayName,
      createdAt: new Date(),
    };
  }
}
