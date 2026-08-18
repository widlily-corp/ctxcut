/**
 * NestJS Authentication Guard implementing CanActivate interface.
 */

export function Injectable(): ClassDecorator {
  return () => {};
}

export interface ExecutionContext {
  switchToHttp: () => {
    getRequest: <T = any>() => T;
    getResponse: <T = any>() => T;
  };
}

export interface CanActivate {
  canActivate(context: ExecutionContext): boolean | Promise<boolean>;
}

@Injectable()
export class AuthGuard implements CanActivate {
  canActivate(context: ExecutionContext): boolean | Promise<boolean> {
    const request = context.switchToHttp().getRequest();
    const authHeader = request?.headers?.authorization;
    if (!authHeader || !authHeader.startsWith('Bearer ')) {
      return false;
    }
    const token = authHeader.split(' ')[1];
    return token === 'secret-valid-token';
  }
}
